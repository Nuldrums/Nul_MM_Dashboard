use std::sync::Arc;
use std::sync::atomic::Ordering;
use axum::{extract::{Path, State}, http::StatusCode, routing::{get, post, put, delete}, Json, Router};
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize)]
pub struct ManualMetricInput {
    pub snapshot_date: Option<String>,
    pub views: Option<i64>,
    pub impressions: Option<i64>,
    pub likes: Option<i64>,
    pub dislikes: Option<i64>,
    pub comments: Option<i64>,
    pub shares: Option<i64>,
    pub saves: Option<i64>,
    pub clicks: Option<i64>,
    pub watch_time_seconds: Option<i64>,
    pub followers_gained: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/posts/{post_id}/metrics", get(get_post_metrics).post(add_manual_metric))
        .route("/api/metrics/{metric_id}", put(update_metric).delete(delete_metric))
        .route("/api/campaigns/{campaign_id}/metrics", get(get_campaign_metrics))
        .route("/api/campaigns/{campaign_id}/metrics/timeline", get(campaign_metrics_timeline))
        .route("/api/campaigns/{campaign_id}/metrics/platforms", get(campaign_metrics_platforms))
        .route("/api/campaigns/{campaign_id}/metrics/post-types", get(campaign_metrics_post_types))
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

    // Sum the latest snapshot per post (snapshots are cumulative totals, not deltas)
    let metrics: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(latest.views),0), COALESCE(SUM(latest.impressions),0),
                COALESCE(SUM(latest.likes),0), COALESCE(SUM(latest.dislikes),0),
                COALESCE(SUM(latest.comments),0), COALESCE(SUM(latest.shares),0),
                COALESCE(SUM(latest.saves),0), COALESCE(SUM(latest.clicks),0),
                (SELECT COUNT(*) FROM metric_snapshots ms2 JOIN posts p2 ON ms2.post_id = p2.id WHERE p2.campaign_id = ?)
         FROM posts p
         JOIN (
             SELECT post_id, views, impressions, likes, dislikes, comments, shares, saves, clicks,
                    ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY snapshot_date DESC) AS rn
             FROM metric_snapshots
         ) latest ON latest.post_id = p.id AND latest.rn = 1
         WHERE p.campaign_id = ?"
    ).bind(&campaign_id).bind(&campaign_id).fetch_one(&state.db).await?;

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

async fn campaign_metrics_timeline(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT ms.snapshot_date,
                COALESCE(SUM(ms.views), 0)    AS views,
                COALESCE(SUM(ms.likes), 0)    AS likes,
                COALESCE(SUM(ms.comments), 0) AS comments,
                COALESCE(SUM(ms.shares), 0)   AS shares,
                COUNT(DISTINCT ms.post_id)    AS posts
         FROM metric_snapshots ms JOIN posts p ON ms.post_id = p.id
         WHERE p.campaign_id = ?
         GROUP BY ms.snapshot_date
         ORDER BY ms.snapshot_date ASC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    let result: Vec<serde_json::Value> = rows.into_iter().map(|(date, views, likes, comments, shares, posts)| {
        serde_json::json!({
            "date": date,
            "views": views,
            "likes": likes,
            "comments": comments,
            "shares": shares,
            "engagement": likes + comments + shares,
            "posts": posts,
        })
    }).collect();

    Ok(Json(result))
}

async fn campaign_metrics_platforms(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    // Use latest snapshot per post — snapshots are cumulative point-in-time totals
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT p.platform,
                COUNT(DISTINCT p.id)              AS post_count,
                COALESCE(SUM(latest.views), 0)    AS views,
                COALESCE(SUM(latest.likes), 0)    AS likes,
                COALESCE(SUM(latest.comments), 0) AS comments,
                COALESCE(SUM(latest.shares), 0)   AS shares
         FROM posts p
         LEFT JOIN (
             SELECT post_id, views, likes, comments, shares,
                    ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY snapshot_date DESC) AS rn
             FROM metric_snapshots
         ) latest ON latest.post_id = p.id AND latest.rn = 1
         WHERE p.campaign_id = ?
         GROUP BY p.platform
         ORDER BY post_count DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    let result: Vec<serde_json::Value> = rows.into_iter().map(|(platform, post_count, views, likes, comments, shares)| {
        serde_json::json!({
            "platform": platform,
            "post_count": post_count,
            "views": views,
            "likes": likes,
            "comments": comments,
            "shares": shares,
            "engagement": likes + comments + shares,
        })
    }).collect();

    Ok(Json(result))
}

async fn campaign_metrics_post_types(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    // Use latest snapshot per post — snapshots are cumulative point-in-time totals
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT p.post_type,
                COUNT(DISTINCT p.id) AS post_count,
                COALESCE(SUM(latest.likes) + SUM(latest.comments) + SUM(latest.shares), 0) AS engagement
         FROM posts p
         LEFT JOIN (
             SELECT post_id, likes, comments, shares,
                    ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY snapshot_date DESC) AS rn
             FROM metric_snapshots
         ) latest ON latest.post_id = p.id AND latest.rn = 1
         WHERE p.campaign_id = ?
         GROUP BY p.post_type
         ORDER BY engagement DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    let result: Vec<serde_json::Value> = rows.into_iter().map(|(post_type, post_count, engagement)| {
        serde_json::json!({
            "post_type": post_type,
            "post_count": post_count,
            "engagement": engagement,
        })
    }).collect();

    Ok(Json(result))
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

async fn add_manual_metric(
    State(state): State<Arc<AppState>>,
    Path(post_id): Path<String>,
    Json(data): Json<ManualMetricInput>,
) -> Result<(StatusCode, Json<MetricSnapshotResponse>), AppError> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM posts WHERE id = ?"
    ).bind(&post_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Post not found".into()));
    }

    let snapshot_date = data.snapshot_date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    sqlx::query(
        "INSERT INTO metric_snapshots (post_id, snapshot_date, views, impressions, likes, dislikes,
         comments, shares, saves, clicks, watch_time_seconds, followers_gained, fetched_via)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(post_id, snapshot_date) DO UPDATE SET
           views=excluded.views, impressions=excluded.impressions, likes=excluded.likes,
           dislikes=excluded.dislikes, comments=excluded.comments, shares=excluded.shares,
           saves=excluded.saves, clicks=excluded.clicks, watch_time_seconds=excluded.watch_time_seconds,
           followers_gained=excluded.followers_gained, fetched_via=excluded.fetched_via"
    )
        .bind(&post_id).bind(&snapshot_date)
        .bind(data.views.unwrap_or(0)).bind(data.impressions.unwrap_or(0))
        .bind(data.likes.unwrap_or(0)).bind(data.dislikes.unwrap_or(0))
        .bind(data.comments.unwrap_or(0)).bind(data.shares.unwrap_or(0))
        .bind(data.saves.unwrap_or(0)).bind(data.clicks.unwrap_or(0))
        .bind(data.watch_time_seconds).bind(data.followers_gained.unwrap_or(0))
        .bind("manual")
        .execute(&state.db).await?;

    let row = sqlx::query_as::<_, MetricSnapshotRow>(
        "SELECT id, post_id, snapshot_date, views, impressions, likes, dislikes, comments,
                shares, saves, clicks, watch_time_seconds, followers_gained, custom_metrics,
                fetched_via, created_at
         FROM metric_snapshots WHERE post_id = ? AND snapshot_date = ?"
    ).bind(&post_id).bind(&snapshot_date).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn update_metric(
    State(state): State<Arc<AppState>>,
    Path(metric_id): Path<i64>,
    Json(data): Json<ManualMetricInput>,
) -> Result<Json<MetricSnapshotResponse>, AppError> {
    let existing = sqlx::query_as::<_, MetricSnapshotRow>(
        "SELECT id, post_id, snapshot_date, views, impressions, likes, dislikes, comments,
                shares, saves, clicks, watch_time_seconds, followers_gained, custom_metrics,
                fetched_via, created_at
         FROM metric_snapshots WHERE id = ?"
    ).bind(metric_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Metric snapshot not found".into()));
    }
    let row = existing.unwrap();

    let snapshot_date = data.snapshot_date.unwrap_or(row.snapshot_date);
    let views = data.views.unwrap_or(row.views.unwrap_or(0));
    let impressions = data.impressions.unwrap_or(row.impressions.unwrap_or(0));
    let likes = data.likes.unwrap_or(row.likes.unwrap_or(0));
    let dislikes = data.dislikes.unwrap_or(row.dislikes.unwrap_or(0));
    let comments = data.comments.unwrap_or(row.comments.unwrap_or(0));
    let shares = data.shares.unwrap_or(row.shares.unwrap_or(0));
    let saves = data.saves.unwrap_or(row.saves.unwrap_or(0));
    let clicks = data.clicks.unwrap_or(row.clicks.unwrap_or(0));
    let watch_time_seconds = data.watch_time_seconds.or(row.watch_time_seconds);
    let followers_gained = data.followers_gained.unwrap_or(row.followers_gained.unwrap_or(0));

    sqlx::query(
        "UPDATE metric_snapshots SET snapshot_date=?, views=?, impressions=?, likes=?, dislikes=?,
         comments=?, shares=?, saves=?, clicks=?, watch_time_seconds=?, followers_gained=?
         WHERE id=?"
    )
        .bind(&snapshot_date).bind(views).bind(impressions).bind(likes).bind(dislikes)
        .bind(comments).bind(shares).bind(saves).bind(clicks)
        .bind(watch_time_seconds).bind(followers_gained)
        .bind(metric_id)
        .execute(&state.db).await?;

    let updated = sqlx::query_as::<_, MetricSnapshotRow>(
        "SELECT id, post_id, snapshot_date, views, impressions, likes, dislikes, comments,
                shares, saves, clicks, watch_time_seconds, followers_gained, custom_metrics,
                fetched_via, created_at
         FROM metric_snapshots WHERE id = ?"
    ).bind(metric_id).fetch_one(&state.db).await?;

    Ok(Json(updated.into()))
}

async fn delete_metric(
    State(state): State<Arc<AppState>>,
    Path(metric_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM metric_snapshots WHERE id = ?"
    ).bind(metric_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Metric snapshot not found".into()));
    }

    sqlx::query("DELETE FROM metric_snapshots WHERE id = ?")
        .bind(metric_id).execute(&state.db).await?;

    Ok(StatusCode::NO_CONTENT)
}
