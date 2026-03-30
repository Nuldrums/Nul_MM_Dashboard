use std::sync::Arc;
use std::sync::atomic::Ordering;
use axum::{extract::{Path, Query, State}, routing::{get, post}, Json, Router};
use serde::Deserialize;
use crate::server::{AppState, error::AppError};
use crate::server::db::models::{AIAnalysisRow, parse_json_column};
use crate::server::services::daily_pipeline;
use crate::server::ai::embedder;

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
        .route("/api/ai/status", get(analysis_status))
        .route("/api/ai/recommendations", get(recommendations))
        .route("/api/ai/knowledge-base/query", get(knowledge_base_query))
        .route("/api/ai/knowledge-base/stats", get(knowledge_base_stats))
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

    if state.settings.anthropic_api_key.is_empty() {
        return Json(serde_json::json!({
            "message": "No Anthropic API key configured. Set ANTHROPIC_API_KEY in .env.",
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

    Ok(Json(serde_json::json!({
        "running": running,
        "last_run": last.as_ref().and_then(|r| r.0.as_ref()),
        "api_key_configured": !state.settings.anthropic_api_key.is_empty(),
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

async fn knowledge_base_query(
    State(state): State<Arc<AppState>>,
    Query(params): Query<KbQueryParams>,
) -> Json<serde_json::Value> {
    let q = params.q.unwrap_or_default();
    if q.is_empty() {
        return Json(serde_json::json!({"query": q, "results": [], "message": "Provide a query parameter 'q'."}));
    }

    match embedder::query(&state.db, &q, 10).await {
        Ok(results) => {
            let count = results.len();
            Json(serde_json::json!({"query": q, "results": results, "count": count}))
        }
        Err(e) => {
            tracing::error!("Knowledge base query error: {}", e);
            Json(serde_json::json!({"query": q, "results": [], "message": format!("Query failed: {}", e)}))
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
