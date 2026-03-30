use std::sync::Arc;
use crate::server::AppState;
use crate::server::ai::{analyzer, embedder};
use crate::server::services::metric_collector;

pub async fn run_full(state: &Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting daily pipeline");

    // Step 1: Fetch metrics
    tracing::info!("[pipeline] Step 1: Fetching metrics");
    if let Err(e) = metric_collector::collect_all(state).await {
        tracing::error!("[pipeline] Metric collection failed: {}", e);
        // Continue with analysis even if metrics fail
    }

    // Step 2: Per-campaign AI analysis
    tracing::info!("[pipeline] Step 2: Running campaign analyses");
    let campaigns: Vec<(String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT c.id, c.name, c.goal, c.target_audience, p.name
         FROM campaigns c JOIN products p ON c.product_id = p.id
         WHERE c.status = 'active'"
    ).fetch_all(&state.db).await?;

    let mut analysis_ids = Vec::new();

    for (campaign_id, campaign_name, goal, target_audience, product_name) in &campaigns {
        // Build context
        let posts_data = build_posts_data(&state.db, campaign_id).await?;

        let context = serde_json::json!({
            "campaign_name": campaign_name,
            "product_name": product_name,
            "product_type": "",
            "goal": goal.as_deref().unwrap_or(""),
            "target_audience": target_audience.as_deref().unwrap_or(""),
            "duration_days": 30,
            "posts_data": posts_data,
            "historical_context": "No historical data available.",
        });

        match analyzer::analyze_campaign(state, &context).await {
            Ok(result) => {
                let analysis_id = uuid::Uuid::new_v4().to_string();
                let analysis = &result["analysis"];
                let summary = analysis["summary"].as_str().unwrap_or("Analysis complete");

                sqlx::query(
                    "INSERT INTO ai_analyses (id, campaign_id, analysis_type, summary, top_performers,
                     underperformers, patterns, recommendations, raw_response, model_used, tokens_used)
                     VALUES (?, ?, 'campaign_daily', ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                    .bind(&analysis_id).bind(campaign_id).bind(summary)
                    .bind(analysis["top_performers"].to_string())
                    .bind(analysis["underperformers"].to_string())
                    .bind(analysis["patterns"].to_string())
                    .bind(analysis["recommendations"].to_string())
                    .bind(result["raw_response"].as_str())
                    .bind(result["model_used"].as_str())
                    .bind(result["tokens_used"].as_i64())
                    .execute(&state.db).await?;

                analysis_ids.push(analysis_id.clone());

                // Step 4: Embed high-signal posts
                if let Some(performers) = analysis["top_performers"].as_array() {
                    for p in performers {
                        let _ = embedder::embed_post_insight(&state.db, p["post_id"].as_str().unwrap_or(""), p).await;
                    }
                }
                if let Some(performers) = analysis["underperformers"].as_array() {
                    for p in performers {
                        let _ = embedder::embed_post_insight(&state.db, p["post_id"].as_str().unwrap_or(""), p).await;
                    }
                }

                // Step 5: Embed patterns
                if let Some(patterns) = analysis["patterns"].as_array() {
                    for pat in patterns {
                        if pat["confidence"].as_str() == Some("high") {
                            let _ = embedder::embed_pattern(&state.db, pat, &analysis_id).await;
                        }
                    }
                }

                tracing::info!("[pipeline] Campaign '{}' analysis complete", campaign_name);
            }
            Err(e) => {
                tracing::error!("[pipeline] Campaign '{}' analysis failed: {}", campaign_name, e);
            }
        }
    }

    // Step 3: Cross-campaign analysis (if >1 active campaigns)
    if campaigns.len() > 1 {
        tracing::info!("[pipeline] Step 3: Running cross-campaign analysis");
        let campaigns_summary = campaigns.iter()
            .map(|(id, name, goal, _, _)| format!("- {} ({}): {}", name, id, goal.as_deref().unwrap_or("no goal")))
            .collect::<Vec<_>>()
            .join("\n");

        match analyzer::analyze_cross_campaign(state, &campaigns_summary, "No historical patterns yet.").await {
            Ok(result) => {
                let analysis_id = uuid::Uuid::new_v4().to_string();
                let analysis = &result["analysis"];
                let summary = analysis["summary"].as_str().unwrap_or("Cross-campaign analysis complete");

                sqlx::query(
                    "INSERT INTO ai_analyses (id, campaign_id, analysis_type, summary, top_performers,
                     underperformers, patterns, recommendations, raw_response, model_used, tokens_used)
                     VALUES (?, NULL, 'cross_campaign', ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                    .bind(&analysis_id).bind(summary)
                    .bind(analysis["top_performers"].to_string())
                    .bind(analysis["underperformers"].to_string())
                    .bind(analysis["patterns"].to_string())
                    .bind(analysis["recommendations"].to_string())
                    .bind(result["raw_response"].as_str())
                    .bind(result["model_used"].as_str())
                    .bind(result["tokens_used"].as_i64())
                    .execute(&state.db).await?;

                tracing::info!("[pipeline] Cross-campaign analysis complete");
            }
            Err(e) => {
                tracing::error!("[pipeline] Cross-campaign analysis failed: {}", e);
            }
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
