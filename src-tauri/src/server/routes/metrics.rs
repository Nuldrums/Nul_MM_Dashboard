use std::sync::Arc;
use std::sync::atomic::Ordering;
use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use serde::Serialize;
use crate::server::{AppState, error::AppError};
use crate::server::db::models::MetricSnapshotRow;
use crate::server::services::metric_collector;

#[derive(Serialize)]
pub struct MetricSnapshotResponse {
    pub id: i64,
    pub post_id: String,
    pub snapshot_date: String,
    pub views: i64,
    pub impressions: i64,
    pub likes: i64,
    pub dislikes: i64,
    pub comments: i64,
    pub shares: i64,
    pub saves: i64,
    pub clicks: i64,
    pub watch_time_seconds: Option<i64>,
    pub followers_gained: i64,
    pub custom_metrics: Option<serde_json::Value>,
    pub fetched_via: Option<String>,
    pub created_at: Option<String>,
}

impl From<MetricSnapshotRow> for MetricSnapshotResponse {
    fn from(r: MetricSnapshotRow) -> Self {
        Self {
            id: r.id,
            post_id: r.post_id,
            snapshot_date: r.snapshot_date,
            views: r.views.unwrap_or(0),
            impressions: r.impressions.unwrap_or(0),
            likes: r.likes.unwrap_or(0),
            dislikes: r.dislikes.unwrap_or(0),
            comments: r.comments.unwrap_or(0),
            shares: r.shares.unwrap_or(0),
            saves: r.saves.unwrap_or(0),
            clicks: r.clicks.unwrap_or(0),
            watch_time_seconds: r.watch_time_seconds,
            followers_gained: r.followers_gained.unwrap_or(0),
            custom_metrics: r.custom_metrics.and_then(|s| serde_json::from_str(&s).ok()),
            fetched_via: r.fetched_via,
            created_at: r.created_at.map(|dt| dt.to_string()),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/posts/{post_id}/metrics", get(get_post_metrics))
        .route("/api/campaigns/{campaign_id}/metrics", get(get_campaign_metrics))
        .route("/api/metrics/fetch", post(trigger_fetch))
        .route("/api/metrics/fetch/status", get(fetch_status))
}

async fn get_post_metrics(
    State(state): State<Arc<AppState>>,
    Path(post_id): Path<String>,
) -> Result<Json<Vec<MetricSnapshotResponse>>, AppError> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM posts WHERE id = ?"
    ).bind(&post_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Post not found".into()));
    }

    let rows = sqlx::query_as::<_, MetricSnapshotRow>(
        "SELECT id, post_id, snapshot_date, views, impressions, likes, dislikes, comments,
                shares, saves, clicks, watch_time_seconds, followers_gained, custom_metrics,
                fetched_via, created_at
         FROM metric_snapshots WHERE post_id = ? ORDER BY snapshot_date ASC"
    ).bind(&post_id).fetch_all(&state.db).await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn get_campaign_metrics(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Campaign not found".into()));
    }

    let metrics: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(ms.views),0), COALESCE(SUM(ms.impressions),0),
                COALESCE(SUM(ms.likes),0), COALESCE(SUM(ms.dislikes),0),
                COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0),
                COALESCE(SUM(ms.saves),0), COALESCE(SUM(ms.clicks),0),
                COUNT(ms.id)
         FROM metric_snapshots ms JOIN posts p ON ms.post_id = p.id
         WHERE p.campaign_id = ?"
    ).bind(&campaign_id).fetch_one(&state.db).await?;

    let (post_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM posts WHERE campaign_id = ?"
    ).bind(&campaign_id).fetch_one(&state.db).await?;

    Ok(Json(serde_json::json!({
        "campaign_id": campaign_id,
        "total_views": metrics.0,
        "total_impressions": metrics.1,
        "total_likes": metrics.2,
        "total_dislikes": metrics.3,
        "total_comments": metrics.4,
        "total_shares": metrics.5,
        "total_saves": metrics.6,
        "total_clicks": metrics.7,
        "snapshot_count": metrics.8,
        "post_count": post_count,
    })))
}

async fn trigger_fetch(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    if state.fetch_running.load(Ordering::Relaxed) {
        return Json(serde_json::json!({"message": "Fetch already in progress", "status": "running"}));
    }

    state.fetch_running.store(true, Ordering::Relaxed);
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = metric_collector::collect_all(&state_clone).await {
            tracing::error!("Metric fetch failed: {}", e);
        }
        state_clone.fetch_running.store(false, Ordering::Relaxed);
    });

    Json(serde_json::json!({"message": "Metric fetch started", "status": "started"}))
}

async fn fetch_status(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let running = state.fetch_running.load(Ordering::Relaxed);

    let last: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'last_metric_fetch'"
    ).fetch_optional(&state.db).await.unwrap_or(None);

    Json(serde_json::json!({
        "running": running,
        "last_completed": last.and_then(|r| r.0),
    }))
}
