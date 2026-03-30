use std::sync::Arc;
use axum::{extract::{Query, State}, routing::get, Json, Router};
use serde::Deserialize;
use crate::server::{AppState, error::AppError};

#[derive(Deserialize)]
pub struct ProfileIdFilter {
    pub profile_id: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/analytics/overview", get(overview))
        .route("/api/analytics/platforms", get(platforms))
        .route("/api/analytics/post-types", get(post_types))
        .route("/api/analytics/trends", get(trends))
}

async fn overview(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (campaign_count,): (i64,) = if let Some(ref pid) = params.profile_id {
        sqlx::query_as("SELECT COUNT(*) FROM campaigns WHERE status != 'archived' AND profile_id = ?")
            .bind(pid).fetch_one(&state.db).await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM campaigns WHERE status != 'archived'")
            .fetch_one(&state.db).await?
    };

    let (post_count,): (i64,) = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT COUNT(*) FROM posts p JOIN campaigns c ON p.campaign_id = c.id WHERE c.profile_id = ?"
        ).bind(pid).fetch_one(&state.db).await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM posts").fetch_one(&state.db).await?
    };

    let (product_count,): (i64,) = if let Some(ref pid) = params.profile_id {
        sqlx::query_as("SELECT COUNT(*) FROM products WHERE profile_id = ?")
            .bind(pid).fetch_one(&state.db).await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM products").fetch_one(&state.db).await?
    };

    let metrics: (i64, i64, i64, i64) = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT COALESCE(SUM(ms.views),0), COALESCE(SUM(ms.likes),0),
                    COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0)
             FROM metric_snapshots ms
             JOIN posts p ON ms.post_id = p.id
             JOIN campaigns c ON p.campaign_id = c.id
             WHERE c.profile_id = ?"
        ).bind(pid).fetch_one(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT COALESCE(SUM(views),0), COALESCE(SUM(likes),0),
                    COALESCE(SUM(comments),0), COALESCE(SUM(shares),0)
             FROM metric_snapshots"
        ).fetch_one(&state.db).await?
    };

    // Top 5 posts by likes
    let top_posts: Vec<(String, Option<String>, String, i64)> = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT p.id, p.title, p.platform, COALESCE(SUM(ms.likes),0) as total_likes
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             JOIN campaigns c ON p.campaign_id = c.id WHERE c.profile_id = ?
             GROUP BY p.id ORDER BY total_likes DESC LIMIT 5"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT p.id, p.title, p.platform, COALESCE(SUM(ms.likes),0) as total_likes
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             GROUP BY p.id ORDER BY total_likes DESC LIMIT 5"
        ).fetch_all(&state.db).await?
    };

    let top_posts_json: Vec<serde_json::Value> = top_posts.into_iter().map(|(id, title, platform, likes)| {
        serde_json::json!({"id": id, "title": title, "platform": platform, "total_likes": likes})
    }).collect();

    Ok(Json(serde_json::json!({
        "active_campaigns": campaign_count,
        "total_posts": post_count,
        "total_products": product_count,
        "total_views": metrics.0,
        "total_likes": metrics.1,
        "total_comments": metrics.2,
        "total_shares": metrics.3,
        "top_posts": top_posts_json,
    })))
}

async fn platforms(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT p.platform, COUNT(DISTINCT p.id), COALESCE(SUM(ms.views),0),
                    COALESCE(SUM(ms.likes),0), COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0)
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             JOIN campaigns c ON p.campaign_id = c.id WHERE c.profile_id = ?
             GROUP BY p.platform ORDER BY COUNT(DISTINCT p.id) DESC"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT p.platform, COUNT(DISTINCT p.id), COALESCE(SUM(ms.views),0),
                    COALESCE(SUM(ms.likes),0), COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0)
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             GROUP BY p.platform ORDER BY COUNT(DISTINCT p.id) DESC"
        ).fetch_all(&state.db).await?
    };

    let result: Vec<serde_json::Value> = rows.into_iter().map(|r| {
        serde_json::json!({
            "platform": r.0, "post_count": r.1, "views": r.2,
            "likes": r.3, "comments": r.4, "shares": r.5,
        })
    }).collect();

    Ok(Json(result))
}

async fn post_types(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT p.post_type, COUNT(DISTINCT p.id), COALESCE(SUM(ms.views),0),
                    COALESCE(SUM(ms.likes),0), COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0)
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             JOIN campaigns c ON p.campaign_id = c.id WHERE c.profile_id = ?
             GROUP BY p.post_type ORDER BY COUNT(DISTINCT p.id) DESC"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT p.post_type, COUNT(DISTINCT p.id), COALESCE(SUM(ms.views),0),
                    COALESCE(SUM(ms.likes),0), COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0)
             FROM posts p LEFT JOIN metric_snapshots ms ON ms.post_id = p.id
             GROUP BY p.post_type ORDER BY COUNT(DISTINCT p.id) DESC"
        ).fetch_all(&state.db).await?
    };

    let result: Vec<serde_json::Value> = rows.into_iter().map(|r| {
        serde_json::json!({
            "post_type": r.0, "post_count": r.1, "views": r.2,
            "likes": r.3, "comments": r.4, "shares": r.5,
        })
    }).collect();

    Ok(Json(result))
}

async fn trends(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = if let Some(ref pid) = params.profile_id {
        sqlx::query_as(
            "SELECT ms.snapshot_date, COALESCE(SUM(ms.views),0), COALESCE(SUM(ms.likes),0),
                    COALESCE(SUM(ms.comments),0), COALESCE(SUM(ms.shares),0), COUNT(ms.id)
             FROM metric_snapshots ms
             JOIN posts p ON ms.post_id = p.id
             JOIN campaigns c ON p.campaign_id = c.id
             WHERE c.profile_id = ?
             GROUP BY ms.snapshot_date ORDER BY ms.snapshot_date ASC"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as(
            "SELECT snapshot_date, COALESCE(SUM(views),0), COALESCE(SUM(likes),0),
                    COALESCE(SUM(comments),0), COALESCE(SUM(shares),0), COUNT(id)
             FROM metric_snapshots GROUP BY snapshot_date ORDER BY snapshot_date ASC"
        ).fetch_all(&state.db).await?
    };

    let result: Vec<serde_json::Value> = rows.into_iter().map(|r| {
        serde_json::json!({
            "date": r.0, "views": r.1, "likes": r.2,
            "comments": r.3, "shares": r.4, "snapshot_count": r.5,
        })
    }).collect();

    Ok(Json(result))
}
