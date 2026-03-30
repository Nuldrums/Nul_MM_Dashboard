use sqlx::SqlitePool;

/// FTS5-based knowledge base (replaces ChromaDB).
/// Provides keyword search over campaign learnings, post insights, and patterns.

pub async fn insert_document(
    pool: &SqlitePool,
    doc_id: &str,
    doc_type: &str,
    content: &str,
    metadata: &serde_json::Value,
) -> anyhow::Result<()> {
    // Delete existing document with same ID (upsert)
    sqlx::query("DELETE FROM knowledge_fts WHERE doc_id = ?")
        .bind(doc_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO knowledge_fts (doc_id, doc_type, content, metadata) VALUES (?, ?, ?, ?)"
    )
        .bind(doc_id)
        .bind(doc_type)
        .bind(content)
        .bind(metadata.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn query(
    pool: &SqlitePool,
    query_text: &str,
    limit: i32,
) -> anyhow::Result<Vec<serde_json::Value>> {
    if query_text.trim().is_empty() {
        return Ok(vec![]);
    }

    // Escape FTS5 special characters and build query
    let fts_query = query_text
        .replace('"', "\"\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" OR ");

    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT doc_id, doc_type, content, metadata FROM knowledge_fts
         WHERE knowledge_fts MATCH ? ORDER BY rank LIMIT ?"
    )
        .bind(&fts_query)
        .bind(limit)
        .fetch_all(pool)
        .await?;

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

pub async fn get_stats(pool: &SqlitePool) -> anyhow::Result<serde_json::Value> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM knowledge_fts"
    ).fetch_one(pool).await?;

    let by_type: Vec<(String, i64)> = sqlx::query_as(
        "SELECT doc_type, COUNT(*) FROM knowledge_fts GROUP BY doc_type"
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

    insert_document(pool, &format!("post_{}", post_id), "post_insight", &content, analysis).await
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
    insert_document(pool, &doc_id, "pattern", &content, pattern).await
}

pub async fn embed_campaign_completion(
    pool: &SqlitePool,
    campaign_id: &str,
    campaign_data: &serde_json::Value,
) -> anyhow::Result<()> {
    let name = campaign_data["name"].as_str().unwrap_or("");
    let summary = campaign_data["summary"].as_str().unwrap_or("");

    let content = format!("Campaign '{}' completion: {}", name, summary);

    insert_document(pool, &format!("campaign_{}", campaign_id), "campaign_completion", &content, campaign_data).await
}
