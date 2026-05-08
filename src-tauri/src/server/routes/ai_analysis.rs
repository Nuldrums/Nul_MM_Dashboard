use std::sync::Arc;
use std::sync::atomic::Ordering;
use axum::{extract::{Path, Query, State}, routing::{get, post}, Json, Router};
use serde::Deserialize;
use crate::server::{AppState, error::AppError};
use crate::server::db::models::{AIAnalysisRow, parse_json_column};
use crate::server::services::daily_pipeline;
use crate::server::ai::{embedder, onnx_embedder, analyzer};

#[derive(Deserialize)]
pub struct ProfileIdFilter {
    pub profile_id: Option<String>,
}

#[derive(Deserialize)]
pub struct KbQueryParams {
    pub q: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/ai/latest", get(latest_analyses))
        .route("/api/ai/campaign/{campaign_id}", get(campaign_analyses))
        .route("/api/ai/trigger", post(trigger_analysis))
        .route("/api/ai/test", post(test_ai_connection))
        .route("/api/ai/status", get(analysis_status))
        .route("/api/ai/recommendations", get(recommendations))
        .route("/api/ai/cross-campaign-insight", get(cross_campaign_insight))
        .route("/api/ai/knowledge-base/query", get(knowledge_base_query))
        .route("/api/ai/knowledge-base/stats", get(knowledge_base_stats))
        .route("/api/ai/campaign-recommendations", post(campaign_recommendations))
        .route("/api/ai/model-status", get(model_status))
}

fn analysis_to_json(a: &AIAnalysisRow, campaign_name: Option<&str>) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "id": a.id,
        "campaign_id": a.campaign_id,
        "analysis_type": a.analysis_type,
        "summary": a.summary,
        "top_performers": parse_json_column(&a.top_performers),
        "underperformers": parse_json_column(&a.underperformers),
        "patterns": parse_json_column(&a.patterns),
        "recommendations": parse_json_column(&a.recommendations),
        "model_used": a.model_used,
        "tokens_used": a.tokens_used,
        "analyzed_at": a.analyzed_at.map(|dt| dt.to_string()),
    });
    if let Some(name) = campaign_name {
        obj["campaign_name"] = serde_json::Value::String(name.to_string());
    }
    obj
}

async fn latest_analyses(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let campaigns: Vec<(String, String)> = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT id, name FROM campaigns WHERE status = 'active' AND profile_id = ?"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT id, name FROM campaigns WHERE status = 'active'"
        ).fetch_all(&state.db).await?
    };

    let mut results = Vec::new();
    for (cid, cname) in &campaigns {
        let analysis = sqlx::query_as::<_, AIAnalysisRow>(
            "SELECT id, campaign_id, analysis_type, summary, top_performers, underperformers,
                    patterns, recommendations, raw_response, model_used, tokens_used, analyzed_at
             FROM ai_analyses WHERE campaign_id = ? ORDER BY analyzed_at DESC LIMIT 1"
        ).bind(cid).fetch_optional(&state.db).await?;

        if let Some(a) = analysis {
            results.push(analysis_to_json(&a, Some(cname)));
        }
    }

    Ok(Json(results))
}

async fn campaign_analyses(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Campaign not found".into()));
    }

    let analyses = sqlx::query_as::<_, AIAnalysisRow>(
        "SELECT id, campaign_id, analysis_type, summary, top_performers, underperformers,
                patterns, recommendations, raw_response, model_used, tokens_used, analyzed_at
         FROM ai_analyses WHERE campaign_id = ? ORDER BY analyzed_at DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    let results: Vec<serde_json::Value> = analyses.iter().map(|a| analysis_to_json(a, None)).collect();
    Ok(Json(results))
}

async fn trigger_analysis(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    if state.analysis_running.load(Ordering::Relaxed) {
        return Json(serde_json::json!({"message": "Analysis already in progress", "status": "running"}));
    }

    let cli_available = crate::server::ai::claude_cli::is_available();
    if state.settings.anthropic_api_key.is_empty() && !cli_available {
        return Json(serde_json::json!({
            "message": "No AI provider available. Install Claude CLI for subscription access, or set ANTHROPIC_API_KEY.",
            "status": "error",
        }));
    }

    state.analysis_running.store(true, Ordering::Relaxed);
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = daily_pipeline::run_full(&state_clone).await {
            tracing::error!("Analysis pipeline failed: {}", e);
        }
        state_clone.analysis_running.store(false, Ordering::Relaxed);
    });

    Json(serde_json::json!({"message": "Analysis pipeline started", "status": "started"}))
}

async fn analysis_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let running = state.analysis_running.load(Ordering::Relaxed);

    let last: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'last_ai_analysis'"
    ).fetch_optional(&state.db).await?;

    let cli_available = crate::server::ai::claude_cli::is_available();

    Ok(Json(serde_json::json!({
        "running": running,
        "last_run": last.as_ref().and_then(|r| r.0.as_ref()),
        "api_key_configured": !state.settings.anthropic_api_key.is_empty(),
        "cli_available": cli_available,
        "next_scheduled": serde_json::Value::Null,
    })))
}

async fn recommendations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let analysis = sqlx::query_as::<_, AIAnalysisRow>(
        "SELECT id, campaign_id, analysis_type, summary, top_performers, underperformers,
                patterns, recommendations, raw_response, model_used, tokens_used, analyzed_at
         FROM ai_analyses WHERE analysis_type = 'cross_campaign'
         ORDER BY analyzed_at DESC LIMIT 1"
    ).fetch_optional(&state.db).await?;

    match analysis {
        Some(a) => Ok(Json(serde_json::json!({
            "recommendations": parse_json_column(&a.recommendations),
            "patterns": parse_json_column(&a.patterns),
            "analyzed_at": a.analyzed_at.map(|dt| dt.to_string()),
        }))),
        None => Ok(Json(serde_json::json!({
            "recommendations": [],
            "message": "No cross-campaign analysis available yet",
        }))),
    }
}

async fn cross_campaign_insight(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let analysis = sqlx::query_as::<_, AIAnalysisRow>(
        "SELECT id, campaign_id, analysis_type, summary, top_performers, underperformers,
                patterns, recommendations, raw_response, model_used, tokens_used, analyzed_at
         FROM ai_analyses WHERE analysis_type = 'cross_campaign'
         ORDER BY analyzed_at DESC LIMIT 1"
    ).fetch_optional(&state.db).await?;

    match analysis {
        Some(a) => Ok(Json(serde_json::json!({ "insight": a.summary }))),
        None => Ok(Json(serde_json::json!({ "insight": "" }))),
    }
}

async fn test_ai_connection(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    use crate::server::ai::claude_cli;

    tracing::info!("[ai-test] Starting AI connection test");

    // Read config from DB
    let provider_row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'ai_provider'"
    ).fetch_optional(&state.db).await.unwrap_or(None);
    let provider = provider_row.and_then(|r| r.0).unwrap_or_else(|| "cli".to_string());
    tracing::info!("[ai-test] Provider from DB: {}", provider);

    let model_row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'ai_model'"
    ).fetch_optional(&state.db).await.unwrap_or(None);

    let cli_available = claude_cli::is_available();
    let api_key_set = !state.settings.anthropic_api_key.is_empty();

    let test_prompt = "Respond with exactly one short sentence confirming you are working. Include the word 'MEEM' in your response.";

    // Decide which path to test
    let result = if provider == "api" || (!cli_available && api_key_set) {
        // Test API
        if !api_key_set {
            return Json(serde_json::json!({
                "success": false,
                "provider": "api",
                "error": "No API key configured.",
            }));
        }
        let api_model = model_row.and_then(|r| r.0).unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        tracing::info!("AI test: calling API (model={})", api_model);

        let body = serde_json::json!({
            "model": api_model,
            "max_tokens": 128,
            "messages": [{"role": "user", "content": test_prompt}]
        });
        match state.http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &state.settings.anthropic_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await.unwrap_or_default();
                let text = json["content"][0]["text"].as_str().unwrap_or("(empty response)").to_string();
                tracing::info!("AI test API success: {}", text);
                Ok(("api", api_model, text))
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                Err(format!("API error {}: {}", status, text))
            }
            Err(e) => Err(format!("API request failed: {}", e)),
        }
    } else if cli_available {
        // Test CLI
        let cli_model = model_row.and_then(|r| r.0).map(|m| match m.as_str() {
            "claude-sonnet-4-20250514" => "claude-sonnet-4-6".to_string(),
            "claude-opus-4-20250514" => "claude-opus-4-6".to_string(),
            "claude-3-5-haiku-20241022" => "claude-3-5-haiku".to_string(),
            other => other.to_string(),
        }).unwrap_or_else(|| "claude-sonnet-4-6".to_string());
        tracing::info!("AI test: calling CLI (model={})", cli_model);

        match claude_cli::call_claude_cli(&cli_model, "", test_prompt).await {
            Ok(text) => {
                tracing::info!("AI test CLI success: {}", text);
                Ok(("cli", cli_model, text))
            }
            Err(claude_cli::CliError::RateLimited(msg)) => {
                tracing::warn!("AI test CLI rate limited: {}", msg);
                Err(format!("CLI rate limited: {}. Configure an API key as fallback.", msg))
            }
            Err(e) => Err(format!("CLI error: {}", e)),
        }
    } else {
        return Json(serde_json::json!({
            "success": false,
            "provider": provider,
            "error": "No AI provider available. Install Claude CLI or set an API key.",
            "cli_available": false,
            "api_key_set": false,
        }));
    };

    match result {
        Ok((used_provider, model, response)) => Json(serde_json::json!({
            "success": true,
            "provider": used_provider,
            "model": model,
            "response": response,
            "cli_available": cli_available,
            "api_key_set": api_key_set,
        })),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "provider": provider,
            "error": error,
            "cli_available": cli_available,
            "api_key_set": api_key_set,
        })),
    }
}

async fn knowledge_base_query(
    State(state): State<Arc<AppState>>,
    Query(params): Query<KbQueryParams>,
) -> Json<serde_json::Value> {
    let q = params.q.unwrap_or_default();
    if q.is_empty() {
        return Json(serde_json::json!({"query": q, "results": [], "message": "Provide a query parameter 'q'."}));
    }

    // Try vector search first, fall back to FTS
    match embedder::semantic_query(&state.db, &state.settings.data_dir, &q, 10, None).await {
        Ok(results) if !results.is_empty() => {
            let count = results.len();
            let items: Vec<serde_json::Value> = results.iter().map(|r| serde_json::json!({
                "id": r.id,
                "doc_type": r.doc_type,
                "document": r.content,
                "metadata": r.metadata,
                "relevance": (r.similarity * 100.0).round(),
            })).collect();
            Json(serde_json::json!({"query": q, "results": items, "count": count, "search_type": "vector"}))
        }
        _ => {
            // Fall back to FTS
            match embedder::query(&state.db, &q, 10).await {
                Ok(results) => {
                    let count = results.len();
                    Json(serde_json::json!({"query": q, "results": results, "count": count, "search_type": "fts"}))
                }
                Err(e) => {
                    tracing::error!("Knowledge base query error: {}", e);
                    Json(serde_json::json!({"query": q, "results": [], "message": format!("Query failed: {}", e)}))
                }
            }
        }
    }
}

async fn knowledge_base_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match embedder::get_stats(&state.db).await {
        Ok(stats) => Json(stats),
        Err(e) => Json(serde_json::json!({
            "initialized": false,
            "total_documents": 0,
            "message": format!("Knowledge base not available: {}", e),
        })),
    }
}

#[derive(Deserialize)]
struct CampaignRecommendationRequest {
    product_name: String,
    product_type: String,
    product_description: Option<String>,
    goal: Option<String>,
    target_audience: Option<String>,
    platforms: Option<Vec<String>>,
}

async fn campaign_recommendations(
    State(state): State<Arc<AppState>>,
    Json(data): Json<CampaignRecommendationRequest>,
) -> Json<serde_json::Value> {
    let platforms_str = data.platforms.as_ref()
        .map(|p| p.join(", "))
        .unwrap_or_default();

    let platform_refs: Vec<&str> = data.platforms.as_ref()
        .map(|p| p.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    // Semantic search for relevant past learnings
    let knowledge_context = match embedder::recommend_for_campaign(
        &state.db,
        &state.settings.data_dir,
        &data.product_type,
        data.target_audience.as_deref().unwrap_or(""),
        &platform_refs,
        data.goal.as_deref().unwrap_or(""),
        20,
    ).await {
        Ok(results) if !results.is_empty() => {
            results.iter()
                .map(|r| format!("- [{}] (relevance: {:.0}%) {}", r.doc_type, r.similarity * 100.0, r.content))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Ok(_) => "No prior campaign data available.".to_string(),
        Err(e) => {
            tracing::warn!("Knowledge base query for recommendations failed: {}", e);
            "Knowledge base unavailable.".to_string()
        }
    };

    let context = serde_json::json!({
        "product_name": data.product_name,
        "product_type": data.product_type,
        "product_description": data.product_description.as_deref().unwrap_or(""),
        "goal": data.goal.as_deref().unwrap_or(""),
        "target_audience": data.target_audience.as_deref().unwrap_or(""),
        "platforms": platforms_str,
        "knowledge_context": knowledge_context,
    });

    match analyzer::recommend_new_campaign(&state, &context).await {
        Ok(result) => {
            let analysis = &result["analysis"];
            Json(serde_json::json!({
                "success": true,
                "recommendations": analysis,
                "model_used": result["model_used"].as_str(),
                "knowledge_base_entries_used": knowledge_context.lines().count(),
            }))
        }
        Err(e) => {
            tracing::error!("Campaign recommendation failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "error": format!("{}", e),
            }))
        }
    }
}

async fn model_status(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let ready = onnx_embedder::is_model_ready(&state.settings.data_dir);
    let kb_stats = embedder::get_stats(&state.db).await.unwrap_or(serde_json::json!({"total_documents": 0}));

    Json(serde_json::json!({
        "onnx_model_ready": ready,
        "model_name": "all-MiniLM-L6-v2",
        "embedding_dimensions": 384,
        "knowledge_base": kb_stats,
    }))
}
