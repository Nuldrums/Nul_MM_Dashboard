use sqlx::SqlitePool;
use crate::server::ai::onnx_embedder::{self, cosine_similarity, vec_to_bytes, bytes_to_vec};

#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeResult {
    pub id: String,
    pub doc_type: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub similarity: f32,
    pub campaign_id: Option<String>,
}

/// Insert a document with its vector embedding into the knowledge base.
pub async fn insert_document(
    pool: &SqlitePool,
    data_dir: &str,
    doc_id: &str,
    doc_type: &str,
    content: &str,
    metadata: &serde_json::Value,
    campaign_id: Option<&str>,
) -> anyhow::Result<()> {
    let model = onnx_embedder::get_model(data_dir).await?;

    let content_owned = content.to_string();
    let embedding = tokio::task::spawn_blocking(move || {
        model.embed_single(&content_owned)
    }).await??;

    let embedding_bytes = vec_to_bytes(&embedding);

    // Upsert: delete existing then insert
    sqlx::query("DELETE FROM knowledge_vectors WHERE id = ?")
        .bind(doc_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO knowledge_vectors (id, doc_type, content, embedding, metadata, campaign_id)
         VALUES (?, ?, ?, ?, ?, ?)"
    )
        .bind(doc_id)
        .bind(doc_type)
        .bind(content)
        .bind(&embedding_bytes)
        .bind(metadata.to_string())
        .bind(campaign_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Semantic vector search — returns results ranked by cosine similarity.
pub async fn semantic_query(
    pool: &SqlitePool,
    data_dir: &str,
    query_text: &str,
    limit: usize,
    doc_type_filter: Option<&str>,
) -> anyhow::Result<Vec<KnowledgeResult>> {
    if query_text.trim().is_empty() {
        return Ok(vec![]);
    }

    let model = onnx_embedder::get_model(data_dir).await?;

    let query_owned = query_text.to_string();
    let query_embedding = tokio::task::spawn_blocking(move || {
        model.embed_single(&query_owned)
    }).await??;

    // Load all vectors (filtered by doc_type if specified)
    let rows: Vec<(String, String, String, Vec<u8>, Option<String>, Option<String>)> =
        if let Some(dtype) = doc_type_filter {
            sqlx::query_as(
                "SELECT id, doc_type, content, embedding, metadata, campaign_id
                 FROM knowledge_vectors WHERE doc_type = ?"
            ).bind(dtype).fetch_all(pool).await?
        } else {
            sqlx::query_as(
                "SELECT id, doc_type, content, embedding, metadata, campaign_id
                 FROM knowledge_vectors"
            ).fetch_all(pool).await?
        };

    let mut scored: Vec<KnowledgeResult> = rows.into_iter().map(|(id, dtype, content, emb_bytes, meta, cid)| {
        let emb = bytes_to_vec(&emb_bytes);
        let sim = cosine_similarity(&query_embedding, &emb);
        KnowledgeResult {
            id,
            doc_type: dtype,
            content,
            metadata: meta.as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null),
            similarity: sim,
            campaign_id: cid,
        }
    }).collect();

    // Sort by similarity descending
    scored.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    Ok(scored)
}

/// Get documents relevant to a new campaign's parameters.
/// Combines multiple targeted queries and deduplicates.
pub async fn recommend_for_campaign(
    pool: &SqlitePool,
    data_dir: &str,
    product_type: &str,
    target_audience: &str,
    platforms: &[&str],
    goal: &str,
    limit: usize,
) -> anyhow::Result<Vec<KnowledgeResult>> {
    // Build a rich query combining all campaign attributes
    let platform_str = platforms.join(", ");
    let query = format!(
        "{} {} {} {}",
        product_type, target_audience, goal, platform_str
    );

    // Single semantic search with the combined query
    let mut results = semantic_query(pool, data_dir, &query, limit * 2, None).await?;

    // Filter out low-relevance results (below 0.3 similarity threshold)
    results.retain(|r| r.similarity > 0.3);
    results.truncate(limit);

    Ok(results)
}

/// Get stats about the knowledge base.
pub async fn get_stats(pool: &SqlitePool) -> anyhow::Result<serde_json::Value> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM knowledge_vectors"
    ).fetch_one(pool).await?;

    let by_type: Vec<(String, i64)> = sqlx::query_as(
        "SELECT doc_type, COUNT(*) FROM knowledge_vectors GROUP BY doc_type"
    ).fetch_all(pool).await?;

    let type_counts: serde_json::Map<String, serde_json::Value> = by_type
        .into_iter()
        .map(|(t, c)| (t, serde_json::Value::Number(c.into())))
        .collect();

    Ok(serde_json::json!({
        "total_documents": total.0,
        "by_type": type_counts,
    }))
}

// --- Legacy FTS5 helpers (kept for backward compat, no longer primary) ---

pub async fn embed_post_insight(
    pool: &SqlitePool,
    post_id: &str,
    analysis: &serde_json::Value,
) -> anyhow::Result<()> {
    let platform = analysis["platform"].as_str().unwrap_or("unknown");
    let post_type = analysis["post_type"].as_str().unwrap_or("post");
    let score = analysis["score"].as_f64().unwrap_or(0.0);
    let reasoning = analysis["reasoning"].as_str().unwrap_or("");

    let content = format!(
        "Post insight for {} {}: score={:.0}, reasoning: {}",
        platform, post_type, score, reasoning
    );

    let doc_id = format!("post_{}", post_id);

    // Write to legacy FTS
    sqlx::query("DELETE FROM knowledge_fts WHERE doc_id = ?")
        .bind(&doc_id).execute(pool).await?;
    sqlx::query(
        "INSERT INTO knowledge_fts (doc_id, doc_type, content, metadata) VALUES (?, ?, ?, ?)"
    ).bind(&doc_id).bind("post_insight").bind(&content).bind(analysis.to_string())
        .execute(pool).await?;

    Ok(())
}

pub async fn embed_pattern(
    pool: &SqlitePool,
    pattern: &serde_json::Value,
    analysis_id: &str,
) -> anyhow::Result<()> {
    let content = format!(
        "Pattern: {}\nEvidence: {}\nInsight: {}",
        pattern["pattern"].as_str().unwrap_or(""),
        pattern["evidence"].as_str().unwrap_or(""),
        pattern["actionable_insight"].as_str().unwrap_or(""),
    );

    let doc_id = format!("pattern_{}_{}", analysis_id, uuid::Uuid::new_v4());

    // Write to legacy FTS
    sqlx::query("DELETE FROM knowledge_fts WHERE doc_id = ?")
        .bind(&doc_id).execute(pool).await?;
    sqlx::query(
        "INSERT INTO knowledge_fts (doc_id, doc_type, content, metadata) VALUES (?, ?, ?, ?)"
    ).bind(&doc_id).bind("pattern").bind(&content).bind(pattern.to_string())
        .execute(pool).await?;

    Ok(())
}

pub async fn embed_campaign_completion(
    pool: &SqlitePool,
    campaign_id: &str,
    campaign_data: &serde_json::Value,
) -> anyhow::Result<()> {
    let name = campaign_data["name"].as_str().unwrap_or("");
    let summary = campaign_data["summary"].as_str().unwrap_or("");
    let content = format!("Campaign '{}' completion: {}", name, summary);
    let doc_id = format!("campaign_{}", campaign_id);

    sqlx::query("DELETE FROM knowledge_fts WHERE doc_id = ?")
        .bind(&doc_id).execute(pool).await?;
    sqlx::query(
        "INSERT INTO knowledge_fts (doc_id, doc_type, content, metadata) VALUES (?, ?, ?, ?)"
    ).bind(&doc_id).bind("campaign_completion").bind(&content).bind(campaign_data.to_string())
        .execute(pool).await?;

    Ok(())
}

/// Build an FTS5 MATCH query from user input: escape special chars, OR-join words.
pub fn build_fts_query(query_text: &str) -> String {
    query_text
        .replace('"', "\"\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Legacy FTS query (kept as fallback).
pub async fn query(
    pool: &SqlitePool,
    query_text: &str,
    limit: i32,
) -> anyhow::Result<Vec<serde_json::Value>> {
    if query_text.trim().is_empty() {
        return Ok(vec![]);
    }

    let fts_query = build_fts_query(query_text);

    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT doc_id, doc_type, content, metadata FROM knowledge_fts
         WHERE knowledge_fts MATCH ? ORDER BY rank LIMIT ?"
    ).bind(&fts_query).bind(limit).fetch_all(pool).await?;

    let results: Vec<serde_json::Value> = rows.into_iter().map(|(id, dtype, content, meta)| {
        serde_json::json!({
            "id": id,
            "doc_type": dtype,
            "document": content,
            "metadata": serde_json::from_str::<serde_json::Value>(&meta).unwrap_or_default(),
        })
    }).collect();

    Ok(results)
}
