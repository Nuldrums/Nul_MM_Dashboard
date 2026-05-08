use std::sync::Arc;
use crate::server::AppState;
use crate::server::ai::{analyzer, embedder};
use crate::server::services::metric_collector;
use crate::server::db::models::AIAnalysisRow;

/// Minimum percentage change across any metric to trigger a re-analysis.
const DELTA_THRESHOLD_PCT: f64 = 5.0;

pub async fn run_full(state: &Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting daily pipeline");

    // Step 1: Fetch metrics
    tracing::info!("[pipeline] Step 1: Fetching metrics");
    if let Err(e) = metric_collector::collect_all(state).await {
        tracing::error!("[pipeline] Metric collection failed: {}", e);
    }

    // Step 2: Per-campaign AI analysis (with delta detection)
    tracing::info!("[pipeline] Step 2: Running campaign analyses (delta-aware)");
    let campaigns: Vec<(String, String, Option<String>, Option<String>, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT c.id, c.name, c.goal, c.target_audience, p.name, p.type, c.tags
         FROM campaigns c JOIN products p ON c.product_id = p.id
         WHERE c.status = 'active'"
    ).fetch_all(&state.db).await?;

    let mut analysis_ids = Vec::new();

    for (campaign_id, campaign_name, goal, target_audience, product_name, product_type, campaign_tags) in &campaigns {
        match run_campaign_analysis(state, campaign_id, campaign_name, goal.as_deref(), target_audience.as_deref(), product_name, product_type.as_deref().unwrap_or(""), campaign_tags.as_deref()).await {
            Ok(Some(aid)) => analysis_ids.push(aid),
            Ok(None) => tracing::info!("[pipeline] Campaign '{}' skipped (no significant changes)", campaign_name),
            Err(e) => tracing::error!("[pipeline] Campaign '{}' analysis failed: {}", campaign_name, e),
        }
    }

    // Step 3: Cross-campaign analysis (if >1 active campaigns)
    if campaigns.len() > 1 {
        tracing::info!("[pipeline] Step 3: Running cross-campaign analysis");

        // Build richer cross-campaign data including latest analysis summaries
        let mut campaigns_data_lines = Vec::new();
        for (cid, cname, goal, _, _, _, _) in &campaigns {
            let latest: Option<(String,)> = sqlx::query_as(
                "SELECT summary FROM ai_analyses WHERE campaign_id = ? AND analysis_type IN ('campaign_daily', 'campaign_delta')
                 ORDER BY analyzed_at DESC LIMIT 1"
            ).bind(cid).fetch_optional(&state.db).await?;

            let summary = latest.map(|r| r.0).unwrap_or_else(|| "No analysis yet".to_string());
            campaigns_data_lines.push(format!("- {} ({}): goal={}, latest analysis: {}",
                cname, cid, goal.as_deref().unwrap_or("none"), summary));
        }
        let campaigns_data = campaigns_data_lines.join("\n");

        // Pull historical cross-campaign patterns from knowledge base
        let historical_patterns = get_cross_campaign_knowledge(state).await;

        match analyzer::analyze_cross_campaign(state, &campaigns_data, &historical_patterns).await {
            Ok(result) => {
                let analysis_id = uuid::Uuid::new_v4().to_string();
                let analysis = &result["analysis"];
                let summary = analysis["summary"].as_str().unwrap_or("Cross-campaign analysis complete");

                sqlx::query(
                    "INSERT INTO ai_analyses (id, campaign_id, analysis_type, summary, top_performers,
                     underperformers, patterns, recommendations, raw_response, model_used, tokens_used, meta_learning)
                     VALUES (?, NULL, 'cross_campaign', ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                    .bind(&analysis_id).bind(summary)
                    .bind(analysis["top_performers"].to_string())
                    .bind(analysis["underperformers"].to_string())
                    .bind(analysis["patterns"].to_string())
                    .bind(analysis["recommendations"].to_string())
                    .bind(result["raw_response"].as_str())
                    .bind(result["model_used"].as_str())
                    .bind(result["tokens_used"].as_i64())
                    .bind(analysis.get("meta_learning").map(|v| v.to_string()))
                    .execute(&state.db).await?;

                tracing::info!("[pipeline] Cross-campaign analysis complete");
            }
            Err(e) => tracing::error!("[pipeline] Cross-campaign analysis failed: {}", e),
        }
    }

    // Update system state
    sqlx::query(
        "INSERT INTO system_state (key, value, updated_at) VALUES ('last_ai_analysis', datetime('now'), datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = datetime('now'), updated_at = datetime('now')"
    ).execute(&state.db).await?;

    tracing::info!("Daily pipeline complete");
    Ok(())
}

/// Run analysis for a single campaign. Returns Some(analysis_id) if analysis ran, None if skipped.
async fn run_campaign_analysis(
    state: &Arc<AppState>,
    campaign_id: &str,
    campaign_name: &str,
    goal: Option<&str>,
    target_audience: Option<&str>,
    product_name: &str,
    product_type: &str,
    campaign_tags: Option<&str>,
) -> anyhow::Result<Option<String>> {
    // Fetch current metrics for all posts
    let current_metrics = get_current_metrics(&state.db, campaign_id).await?;

    // Fetch the most recent prior analysis
    let prior = get_prior_analysis(&state.db, campaign_id).await?;

    let (result, analysis_type) = if let Some(ref prior_analysis) = prior {
        // Delta path: we have a prior analysis
        let prior_snapshots = get_prior_metric_snapshots(&state.db, &prior_analysis.id).await?;
        let delta = compute_delta(&current_metrics, &prior_snapshots);

        // Check if changes are significant enough to re-analyze
        if !delta.has_significant_changes && delta.new_posts.is_empty() {
            tracing::info!("[pipeline] Campaign '{}': no significant metric changes (max delta {:.1}%)",
                campaign_name, delta.max_pct_change);
            return Ok(None);
        }

        let days_since = prior_analysis.analyzed_at
            .map(|dt| (chrono::Utc::now().naive_utc() - dt).num_days())
            .unwrap_or(0);

        // Format prior recommendations as text
        let prior_recs = prior_analysis.recommendations.as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v.as_array().map(|arr| {
                arr.iter().map(|r| format!("- {}", r["action"].as_str().unwrap_or("?")))
                    .collect::<Vec<_>>().join("\n")
            }))
            .unwrap_or_else(|| "None recorded".to_string());

        // Get prior effectiveness score
        let prior_score = prior_analysis.raw_response.as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v["effectiveness_score"].as_i64())
            .unwrap_or(0);

        // Get knowledge context
        let knowledge_context = get_campaign_knowledge(state, product_type, target_audience.unwrap_or(""), goal.unwrap_or("")).await;

        let audience_str = parse_tags_for_ai(target_audience.unwrap_or(""));
        let tags_str = parse_tags_for_ai(campaign_tags.unwrap_or(""));

        let context = serde_json::json!({
            "campaign_name": campaign_name,
            "product_name": product_name,
            "product_type": product_type,
            "goal": goal.unwrap_or(""),
            "target_audience": audience_str,
            "campaign_tags": tags_str,
            "days_since_last": days_since,
            "prior_summary": prior_analysis.summary,
            "prior_score": prior_score,
            "prior_recommendations": prior_recs,
            "metric_deltas": delta.formatted,
            "new_posts_data": delta.new_posts_formatted,
            "knowledge_context": knowledge_context,
        });

        tracing::info!("[pipeline] Campaign '{}': running delta analysis ({}d since last, {} changes, {} new posts)",
            campaign_name, days_since, delta.changed_posts, delta.new_posts.len());

        (analyzer::analyze_campaign_delta(state, &context).await?, "campaign_delta")
    } else {
        // First-run path: full analysis
        let posts_data = build_posts_data(&state.db, campaign_id).await?;
        let knowledge_context = get_campaign_knowledge(state, product_type, target_audience.unwrap_or(""), goal.unwrap_or("")).await;

        let audience_str = parse_tags_for_ai(target_audience.unwrap_or(""));
        let tags_str = parse_tags_for_ai(campaign_tags.unwrap_or(""));

        let context = serde_json::json!({
            "campaign_name": campaign_name,
            "product_name": product_name,
            "product_type": product_type,
            "goal": goal.unwrap_or(""),
            "target_audience": audience_str,
            "campaign_tags": tags_str,
            "duration_days": 30,
            "posts_data": posts_data,
            "historical_context": knowledge_context,
        });

        tracing::info!("[pipeline] Campaign '{}': running first-time full analysis", campaign_name);
        (analyzer::analyze_campaign(state, &context).await?, "campaign_daily")
    };

    // Save analysis
    let analysis_id = uuid::Uuid::new_v4().to_string();
    let analysis = &result["analysis"];
    let summary = analysis["summary"].as_str().unwrap_or("Analysis complete");

    sqlx::query(
        "INSERT INTO ai_analyses (id, campaign_id, analysis_type, summary, top_performers,
         underperformers, patterns, recommendations, raw_response, model_used, tokens_used, meta_learning)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(&analysis_id).bind(campaign_id).bind(analysis_type).bind(summary)
        .bind(analysis["top_performers"].to_string())
        .bind(analysis["underperformers"].to_string())
        .bind(analysis["patterns"].to_string())
        .bind(analysis["recommendations"].to_string())
        .bind(result["raw_response"].as_str())
        .bind(result["model_used"].as_str())
        .bind(result["tokens_used"].as_i64())
        .bind(analysis.get("meta_learning").map(|v| v.to_string()))
        .execute(&state.db).await?;

    // Save current metric snapshots for future delta comparison
    save_metric_snapshots(&state.db, &analysis_id, &current_metrics).await?;

    // Embed learnings to both FTS (legacy) and vector knowledge base
    embed_analysis_learnings(&state.db, &analysis_id, analysis).await;
    embed_vector_learnings(state, &analysis_id, campaign_id, campaign_name, product_type, target_audience.unwrap_or(""), analysis).await;

    tracing::info!("[pipeline] Campaign '{}' analysis saved (type={})", campaign_name, analysis_type);
    Ok(Some(analysis_id))
}

// --- Metric snapshot types ---

struct PostMetrics {
    post_id: String,
    platform: String,
    post_type: String,
    title: String,
    views: i64,
    likes: i64,
    comments: i64,
    shares: i64,
    saves: i64,
    clicks: i64,
}

struct MetricDelta {
    formatted: String,
    new_posts_formatted: String,
    new_posts: Vec<String>,
    changed_posts: usize,
    has_significant_changes: bool,
    max_pct_change: f64,
}

// --- Data fetching helpers ---

async fn get_current_metrics(pool: &sqlx::SqlitePool, campaign_id: &str) -> anyhow::Result<Vec<PostMetrics>> {
    let posts: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, platform, post_type, title FROM posts WHERE campaign_id = ?"
    ).bind(campaign_id).fetch_all(pool).await?;

    let mut results = Vec::new();
    for (pid, platform, post_type, title) in posts {
        let metrics: Option<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT COALESCE(SUM(views),0), COALESCE(SUM(likes),0),
                    COALESCE(SUM(comments),0), COALESCE(SUM(shares),0),
                    COALESCE(SUM(saves),0), COALESCE(SUM(clicks),0)
             FROM metric_snapshots WHERE post_id = ?"
        ).bind(&pid).fetch_optional(pool).await?;

        let (views, likes, comments, shares, saves, clicks) = metrics.unwrap_or((0, 0, 0, 0, 0, 0));
        results.push(PostMetrics {
            post_id: pid,
            platform,
            post_type,
            title: title.unwrap_or_else(|| "untitled".to_string()),
            views, likes, comments, shares, saves, clicks,
        });
    }
    Ok(results)
}

async fn get_prior_analysis(pool: &sqlx::SqlitePool, campaign_id: &str) -> anyhow::Result<Option<AIAnalysisRow>> {
    let row = sqlx::query_as::<_, AIAnalysisRow>(
        "SELECT id, campaign_id, analysis_type, summary, top_performers, underperformers,
                patterns, recommendations, raw_response, model_used, tokens_used, analyzed_at
         FROM ai_analyses
         WHERE campaign_id = ? AND analysis_type IN ('campaign_daily', 'campaign_delta')
         ORDER BY analyzed_at DESC LIMIT 1"
    ).bind(campaign_id).fetch_optional(pool).await?;
    Ok(row)
}

async fn get_prior_metric_snapshots(pool: &sqlx::SqlitePool, analysis_id: &str) -> anyhow::Result<Vec<(String, i64, i64, i64, i64, i64, i64)>> {
    let rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT post_id, COALESCE(views,0), COALESCE(likes,0), COALESCE(comments,0),
                COALESCE(shares,0), COALESCE(saves,0), COALESCE(clicks,0)
         FROM analysis_metric_snapshots WHERE analysis_id = ?"
    ).bind(analysis_id).fetch_all(pool).await?;
    Ok(rows)
}

async fn save_metric_snapshots(pool: &sqlx::SqlitePool, analysis_id: &str, metrics: &[PostMetrics]) -> anyhow::Result<()> {
    for m in metrics {
        sqlx::query(
            "INSERT INTO analysis_metric_snapshots (analysis_id, post_id, views, likes, comments, shares, saves, clicks)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
            .bind(analysis_id).bind(&m.post_id)
            .bind(m.views).bind(m.likes).bind(m.comments)
            .bind(m.shares).bind(m.saves).bind(m.clicks)
            .execute(pool).await?;
    }
    Ok(())
}

// --- Delta computation ---

fn compute_delta(current: &[PostMetrics], prior: &[(String, i64, i64, i64, i64, i64, i64)]) -> MetricDelta {
    use std::collections::HashMap;

    let prior_map: HashMap<&str, (i64, i64, i64, i64, i64, i64)> = prior.iter()
        .map(|(pid, v, l, c, sh, sa, cl)| (pid.as_str(), (*v, *l, *c, *sh, *sa, *cl)))
        .collect();

    let mut delta_lines = Vec::new();
    let mut new_posts_lines = Vec::new();
    let mut new_posts = Vec::new();
    let mut changed_posts = 0;
    let mut max_pct_change: f64 = 0.0;

    for post in current {
        if let Some(&(pv, pl, pc, ps, psa, pcl)) = prior_map.get(post.post_id.as_str()) {
            // Existing post — compute deltas
            let deltas = [
                ("views", post.views - pv, pv),
                ("likes", post.likes - pl, pl),
                ("comments", post.comments - pc, pc),
                ("shares", post.shares - ps, ps),
                ("saves", post.saves - psa, psa),
                ("clicks", post.clicks - pcl, pcl),
            ];

            let mut changes = Vec::new();
            let mut any_change = false;

            for (name, delta, base) in &deltas {
                if *delta != 0 {
                    any_change = true;
                    let pct = if *base > 0 {
                        (*delta as f64 / *base as f64) * 100.0
                    } else if *delta > 0 {
                        100.0 // went from 0 to something
                    } else {
                        0.0
                    };
                    if pct.abs() > max_pct_change {
                        max_pct_change = pct.abs();
                    }
                    let sign = if *delta > 0 { "+" } else { "" };
                    changes.push(format!("{} {}{} ({}{:.0}%)", name, sign, delta, sign, pct));
                }
            }

            if any_change {
                changed_posts += 1;
                delta_lines.push(format!("- [{}] '{}' on {}: {}",
                    post.post_id, post.title, post.platform, changes.join(", ")));
            }
        } else {
            // New post
            new_posts.push(post.post_id.clone());
            new_posts_lines.push(format!(
                "- [{}] {} on {}: '{}' | views={}, likes={}, comments={}, shares={}",
                post.post_id, post.post_type, post.platform, post.title,
                post.views, post.likes, post.comments, post.shares
            ));
        }
    }

    MetricDelta {
        formatted: if delta_lines.is_empty() { "No metric changes detected.".to_string() } else { delta_lines.join("\n") },
        new_posts_formatted: if new_posts_lines.is_empty() { "None".to_string() } else { new_posts_lines.join("\n") },
        new_posts,
        changed_posts,
        has_significant_changes: max_pct_change >= DELTA_THRESHOLD_PCT,
        max_pct_change,
    }
}

/// Parse a JSON array string (e.g. `["a","b"]`) into a comma-separated string for AI prompts.
/// Falls back to returning the input as-is if it's not valid JSON.
fn parse_tags_for_ai(raw: &str) -> String {
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(raw) {
        arr.join(", ")
    } else {
        raw.to_string()
    }
}

// --- Knowledge base helpers ---

async fn get_campaign_knowledge(state: &Arc<AppState>, product_type: &str, audience: &str, goal: &str) -> String {
    let query = format!("{} {} {}", product_type, audience, goal);
    match embedder::semantic_query(&state.db, &state.settings.data_dir, &query, 10, None).await {
        Ok(results) if !results.is_empty() => {
            results.iter()
                .map(|r| format!("- [{}] (relevance: {:.0}%) {}", r.doc_type, r.similarity * 100.0, r.content))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Ok(_) => "No prior knowledge available.".to_string(),
        Err(e) => {
            tracing::warn!("Knowledge base query failed: {}", e);
            "Knowledge base unavailable.".to_string()
        }
    }
}

async fn get_cross_campaign_knowledge(state: &Arc<AppState>) -> String {
    match embedder::semantic_query(&state.db, &state.settings.data_dir, "cross-campaign patterns platform strategy", 15, Some("pattern")).await {
        Ok(results) if !results.is_empty() => {
            results.iter()
                .map(|r| format!("- (relevance: {:.0}%) {}", r.similarity * 100.0, r.content))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Ok(_) => "No historical patterns yet.".to_string(),
        Err(e) => {
            tracing::warn!("Knowledge base query for cross-campaign failed: {}", e);
            "No historical patterns yet.".to_string()
        }
    }
}

// --- Legacy helpers ---

async fn build_posts_data(pool: &sqlx::SqlitePool, campaign_id: &str) -> anyhow::Result<String> {
    let posts: Vec<(String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, platform, post_type, title, url FROM posts WHERE campaign_id = ?"
    ).bind(campaign_id).fetch_all(pool).await?;

    let mut lines = Vec::new();
    for (pid, platform, post_type, title, _url) in &posts {
        let metrics: Option<(i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT COALESCE(SUM(views),0), COALESCE(SUM(likes),0),
                    COALESCE(SUM(comments),0), COALESCE(SUM(shares),0)
             FROM metric_snapshots WHERE post_id = ?"
        ).bind(pid).fetch_optional(pool).await?;

        let (views, likes, comments, shares) = metrics.unwrap_or((0, 0, 0, 0));
        lines.push(format!(
            "- [{}] {} on {}: '{}' | views={}, likes={}, comments={}, shares={}",
            pid, post_type, platform, title.as_deref().unwrap_or("untitled"),
            views, likes, comments, shares
        ));
    }

    Ok(lines.join("\n"))
}

/// Embed learnings into the vector knowledge base for semantic retrieval.
async fn embed_vector_learnings(
    state: &Arc<AppState>,
    analysis_id: &str,
    campaign_id: &str,
    campaign_name: &str,
    product_type: &str,
    target_audience: &str,
    analysis: &serde_json::Value,
) {
    let data_dir = &state.settings.data_dir;
    let base_metadata = serde_json::json!({
        "campaign_id": campaign_id,
        "campaign_name": campaign_name,
        "product_type": product_type,
        "target_audience": target_audience,
        "analysis_id": analysis_id,
    });

    // 1. Embed meta_learning fields
    if let Some(meta) = analysis.get("meta_learning") {
        let fields = [
            ("product_type_insight", "product_type_learning"),
            ("platform_insight", "platform_learning"),
            ("audience_insight", "audience_learning"),
            ("content_format_insight", "content_format_learning"),
        ];
        for (field, doc_type) in &fields {
            if let Some(insight) = meta[field].as_str() {
                if !insight.is_empty() {
                    let doc_id = format!("{}_{}", analysis_id, field);
                    let content = format!("{} for {} campaign targeting {}: {}",
                        field.replace('_', " "), product_type, target_audience, insight);

                    if let Err(e) = embedder::insert_document(
                        &state.db, data_dir, &doc_id, doc_type, &content, &base_metadata, Some(campaign_id)
                    ).await {
                        tracing::warn!("Failed to embed {}: {}", field, e);
                    }
                }
            }
        }
    }

    // 2. Embed high-confidence patterns
    if let Some(patterns) = analysis["patterns"].as_array() {
        for (i, pat) in patterns.iter().enumerate() {
            if pat["confidence"].as_str() == Some("high") || pat["confidence"].as_str() == Some("medium") {
                let doc_id = format!("{}_pattern_{}", analysis_id, i);
                let content = format!(
                    "Pattern from {} campaign ({}): {} Evidence: {} Insight: {}",
                    campaign_name, product_type,
                    pat["pattern"].as_str().unwrap_or(""),
                    pat["evidence"].as_str().unwrap_or(""),
                    pat["actionable_insight"].as_str().unwrap_or("")
                );

                if let Err(e) = embedder::insert_document(
                    &state.db, data_dir, &doc_id, "pattern", &content, &base_metadata, Some(campaign_id)
                ).await {
                    tracing::warn!("Failed to embed pattern: {}", e);
                }
            }
        }
    }

    // 3. Embed campaign summary
    let summary = analysis["summary"].as_str().unwrap_or("");
    let score = analysis["effectiveness_score"].as_i64().unwrap_or(0);
    let doc_id = format!("{}_summary", analysis_id);
    let content = format!(
        "Campaign '{}' for {} product targeting {}: {}. Effectiveness: {}/100",
        campaign_name, product_type, target_audience, summary, score
    );

    if let Err(e) = embedder::insert_document(
        &state.db, data_dir, &doc_id, "campaign_summary", &content, &base_metadata, Some(campaign_id)
    ).await {
        tracing::warn!("Failed to embed campaign summary: {}", e);
    }
}

async fn embed_analysis_learnings(pool: &sqlx::SqlitePool, analysis_id: &str, analysis: &serde_json::Value) {
    // Embed top/bottom performers
    if let Some(performers) = analysis["top_performers"].as_array() {
        for p in performers {
            let _ = embedder::embed_post_insight(pool, p["post_id"].as_str().unwrap_or(""), p).await;
        }
    }
    if let Some(performers) = analysis["underperformers"].as_array() {
        for p in performers {
            let _ = embedder::embed_post_insight(pool, p["post_id"].as_str().unwrap_or(""), p).await;
        }
    }

    // Embed high-confidence patterns
    if let Some(patterns) = analysis["patterns"].as_array() {
        for pat in patterns {
            if pat["confidence"].as_str() == Some("high") {
                let _ = embedder::embed_pattern(pool, pat, analysis_id).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_post(id: &str, platform: &str, views: i64, likes: i64, comments: i64, shares: i64) -> PostMetrics {
        PostMetrics {
            post_id: id.to_string(),
            platform: platform.to_string(),
            post_type: "text".to_string(),
            title: format!("Post {}", id),
            views, likes, comments, shares,
            saves: 0, clicks: 0,
        }
    }

    #[test]
    fn test_compute_delta_no_changes() {
        let current = vec![
            make_post("p1", "reddit", 100, 10, 5, 2),
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(!delta.has_significant_changes);
        assert_eq!(delta.changed_posts, 0);
        assert!(delta.new_posts.is_empty());
        assert_eq!(delta.max_pct_change, 0.0);
    }

    #[test]
    fn test_compute_delta_significant_change() {
        let current = vec![
            make_post("p1", "reddit", 200, 10, 5, 2), // views doubled
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(delta.has_significant_changes);
        assert_eq!(delta.changed_posts, 1);
        assert!(delta.max_pct_change >= 100.0);
        assert!(delta.formatted.contains("+100"));
    }

    #[test]
    fn test_compute_delta_small_change_below_threshold() {
        let current = vec![
            make_post("p1", "reddit", 101, 10, 5, 2), // 1% change
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(!delta.has_significant_changes); // below 5% threshold
        assert_eq!(delta.changed_posts, 1); // still counted as changed
        assert!(delta.max_pct_change < DELTA_THRESHOLD_PCT);
    }

    #[test]
    fn test_compute_delta_new_post() {
        let current = vec![
            make_post("p1", "reddit", 100, 10, 5, 2),
            make_post("p2", "youtube", 50, 3, 1, 0), // new
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert_eq!(delta.new_posts.len(), 1);
        assert_eq!(delta.new_posts[0], "p2");
        assert!(delta.new_posts_formatted.contains("youtube"));
    }

    #[test]
    fn test_compute_delta_all_new() {
        let current = vec![
            make_post("p1", "reddit", 100, 10, 5, 2),
            make_post("p2", "youtube", 50, 3, 1, 0),
        ];
        let prior: Vec<(String, i64, i64, i64, i64, i64, i64)> = vec![];

        let delta = compute_delta(&current, &prior);
        assert_eq!(delta.new_posts.len(), 2);
        assert_eq!(delta.changed_posts, 0);
        assert!(!delta.has_significant_changes); // new posts don't trigger significant flag
    }

    #[test]
    fn test_compute_delta_decline() {
        let current = vec![
            make_post("p1", "reddit", 50, 5, 2, 1), // everything halved
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(delta.has_significant_changes);
        assert!(delta.formatted.contains("-50")); // views went from 100 to 50
    }

    #[test]
    fn test_compute_delta_from_zero() {
        let current = vec![
            make_post("p1", "reddit", 10, 0, 0, 0), // views went from 0 to 10
        ];
        let prior = vec![
            ("p1".to_string(), 0i64, 0, 0, 0, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(delta.has_significant_changes);
        assert_eq!(delta.max_pct_change, 100.0); // 0 -> something = 100%
    }

    #[test]
    fn test_compute_delta_multiple_posts_mixed() {
        let current = vec![
            make_post("p1", "reddit", 100, 10, 5, 2),     // unchanged
            make_post("p2", "youtube", 500, 50, 20, 10),   // big jump
            make_post("p3", "twitter", 30, 2, 1, 0),       // new post
        ];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
            ("p2".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(delta.has_significant_changes);
        assert_eq!(delta.changed_posts, 1); // only p2 changed
        assert_eq!(delta.new_posts.len(), 1); // p3 is new
        assert_eq!(delta.new_posts[0], "p3");
    }

    #[test]
    fn test_compute_delta_empty_current() {
        let current: Vec<PostMetrics> = vec![];
        let prior = vec![
            ("p1".to_string(), 100i64, 10, 5, 2, 0, 0),
        ];

        let delta = compute_delta(&current, &prior);
        assert!(!delta.has_significant_changes);
        assert_eq!(delta.changed_posts, 0);
        assert!(delta.new_posts.is_empty());
    }
}
