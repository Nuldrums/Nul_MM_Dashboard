use std::sync::Arc;
use axum::{extract::{Path, State}, routing::{get, put, post}, Json, Router};
use serde::Deserialize;
use crate::server::{AppState, error::AppError};
use crate::server::db::models::PlatformConfigRow;

const SUPPORTED_PLATFORMS: &[&str] = &[
    "reddit", "youtube", "twitter", "discord",
    "tiktok", "instagram", "linkedin", "other",
];

#[derive(Deserialize)]
pub struct PlatformConfigUpdate {
    pub credentials: Option<serde_json::Value>,
    pub is_enabled: Option<bool>,
    pub config: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct GeneralSettingsUpdate {
    pub data_dir: Option<String>,
    pub auto_fetch_interval_hours: Option<i64>,
    pub auto_analysis_interval_hours: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/settings", get(get_settings))
        .route("/api/settings/platform/{platform_name}", put(update_platform_config))
        .route("/api/settings/general", put(update_general_settings))
        .route("/api/settings/health", get(platform_health))
        .route("/api/system/startup-check", get(startup_check))
        .route("/api/system/export/{campaign_id}", post(export_campaign))
}

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rows = sqlx::query_as::<_, PlatformConfigRow>(
        "SELECT platform, credentials, is_enabled, rate_limit_remaining, last_fetched_at, config
         FROM platform_configs"
    ).fetch_all(&state.db).await?;

    let mut platforms = serde_json::Map::new();
    for pc in rows {
        let creds: serde_json::Value = pc.credentials
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        let creds_obj = creds.as_object().cloned().unwrap_or_default();
        let redacted: serde_json::Map<String, serde_json::Value> = creds_obj.iter().map(|(k, v)| {
            (k.clone(), if v.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                serde_json::json!("***")
            } else {
                serde_json::json!("")
            })
        }).collect();
        let creds_set: serde_json::Map<String, serde_json::Value> = creds_obj.iter().map(|(k, v)| {
            (k.clone(), serde_json::json!(v.as_str().map(|s| !s.is_empty()).unwrap_or(false)))
        }).collect();

        let config_val: serde_json::Value = pc.config
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        platforms.insert(pc.platform, serde_json::json!({
            "is_enabled": pc.is_enabled.unwrap_or(0) != 0,
            "credentials_set": creds_set,
            "credentials_redacted": redacted,
            "last_fetched_at": pc.last_fetched_at.map(|dt| dt.to_string()),
            "config": config_val,
        }));
    }

    // General settings from system_state
    let states: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT key, value FROM system_state"
    ).fetch_all(&state.db).await?;
    let general: serde_json::Map<String, serde_json::Value> = states.into_iter().map(|(k, v)| {
        (k, v.map(|s| serde_json::Value::String(s)).unwrap_or(serde_json::Value::Null))
    }).collect();

    Ok(Json(serde_json::json!({
        "platforms": platforms,
        "general": general,
        "supported_platforms": SUPPORTED_PLATFORMS,
    })))
}

async fn update_platform_config(
    State(state): State<Arc<AppState>>,
    Path(platform_name): Path<String>,
    Json(data): Json<PlatformConfigUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !SUPPORTED_PLATFORMS.contains(&platform_name.as_str()) {
        return Err(AppError::BadRequest(format!("Unsupported platform: {}", platform_name)));
    }

    let is_enabled = data.is_enabled.map(|b| if b { 1 } else { 0 }).unwrap_or(0);
    let creds_str = data.credentials.as_ref().map(|v| v.to_string());
    let config_str = data.config.as_ref().map(|v| v.to_string());

    sqlx::query(
        "INSERT INTO platform_configs (platform, credentials, is_enabled, config)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(platform) DO UPDATE SET
           credentials = COALESCE(excluded.credentials, platform_configs.credentials),
           is_enabled = excluded.is_enabled,
           config = COALESCE(excluded.config, platform_configs.config)"
    )
        .bind(&platform_name)
        .bind(&creds_str)
        .bind(is_enabled)
        .bind(&config_str)
        .execute(&state.db).await?;

    Ok(Json(serde_json::json!({"message": format!("Platform config updated for {}", platform_name)})))
}

async fn update_general_settings(
    State(state): State<Arc<AppState>>,
    Json(data): Json<GeneralSettingsUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let updates: Vec<(&str, String)> = [
        data.data_dir.as_ref().map(|v| ("data_dir", v.clone())),
        data.auto_fetch_interval_hours.map(|v| ("auto_fetch_interval_hours", v.to_string())),
        data.auto_analysis_interval_hours.map(|v| ("auto_analysis_interval_hours", v.to_string())),
    ].into_iter().flatten().collect();

    for (key, value) in updates {
        sqlx::query(
            "INSERT INTO system_state (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')"
        ).bind(key).bind(&value).execute(&state.db).await?;
    }

    Ok(Json(serde_json::json!({"message": "General settings updated"})))
}

async fn platform_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rows = sqlx::query_as::<_, PlatformConfigRow>(
        "SELECT platform, credentials, is_enabled, rate_limit_remaining, last_fetched_at, config
         FROM platform_configs"
    ).fetch_all(&state.db).await?;

    let mut health = serde_json::Map::new();
    for pc in rows {
        let creds: serde_json::Value = pc.credentials
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));
        let has_creds = creds.as_object()
            .map(|o| o.values().any(|v| v.as_str().map(|s| !s.is_empty()).unwrap_or(false)))
            .unwrap_or(false);
        let enabled = pc.is_enabled.unwrap_or(0) != 0;

        health.insert(pc.platform, serde_json::json!({
            "enabled": enabled,
            "credentials_configured": has_creds,
            "last_fetched_at": pc.last_fetched_at.map(|dt| dt.to_string()),
            "status": if has_creds && enabled { "ready" } else { "not_configured" },
        }));
    }

    Ok(Json(serde_json::Value::Object(health)))
}

async fn startup_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let now = chrono::Utc::now().naive_utc();

    let fetch_last: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'last_metric_fetch'"
    ).fetch_optional(&state.db).await?;
    let fetch_val = fetch_last.and_then(|r| r.0);

    let metrics_stale = match &fetch_val {
        Some(ts) => {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                .map(|last| (now - last).num_hours() >= 6)
                .unwrap_or(true)
        }
        None => true,
    };

    let analysis_last: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'last_ai_analysis'"
    ).fetch_optional(&state.db).await?;
    let analysis_val = analysis_last.and_then(|r| r.0);

    let analysis_stale = match &analysis_val {
        Some(ts) => {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                .map(|last| (now - last).num_hours() >= 24)
                .unwrap_or(true)
        }
        None => true,
    };

    Ok(Json(serde_json::json!({
        "metrics_stale": metrics_stale,
        "metrics_last_run": fetch_val,
        "analysis_stale": analysis_stale,
        "analysis_last_run": analysis_val,
        "server_time": now.to_string(),
    })))
}

async fn export_campaign(
    Path(campaign_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Export functionality coming soon",
        "campaign_id": campaign_id,
        "formats_planned": ["json", "csv"],
    }))
}
