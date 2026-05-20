use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, routing::{get, delete}, Json, Router};
use chrono::{Utc, Duration};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::services::metric_collector::build_connectors;

/// On feed creation, anything posted within this grace window gets pulled in on the first
/// discovery tick. Anything older than the window is treated as "already seen" and skipped.
const SEED_GRACE_HOURS: i64 = 24;

#[derive(Deserialize)]
pub struct FeedCreate {
    pub profile_account_id: String,
    pub content_type: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FeedResponse {
    pub id: String,
    pub campaign_id: String,
    pub profile_account_id: String,
    pub platform: String,
    pub account_handle: String,
    pub account_id: Option<String>,
    pub follower_count: Option<i64>,
    pub follower_count_at: Option<String>,
    pub content_type: String,
    pub last_seen_post_id: Option<String>,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub is_active: i32,
    pub created_at: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/campaigns/{campaign_id}/feeds", get(list_feeds).post(create_feed))
        .route("/api/feeds/{feed_id}", delete(delete_feed))
}

async fn list_feeds(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<FeedResponse>>, AppError> {
    let rows: Vec<FeedResponse> = sqlx::query_as(
        "SELECT cf.id, cf.campaign_id, cf.profile_account_id,
                pa.platform, pa.account_handle, pa.account_id,
                pa.follower_count, pa.follower_count_at,
                cf.content_type, cf.last_seen_post_id, cf.last_checked_at,
                cf.last_error, cf.is_active, cf.created_at
         FROM campaign_feeds cf
         JOIN profile_accounts pa ON pa.id = cf.profile_account_id
         WHERE cf.campaign_id = ?
         ORDER BY cf.created_at DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;
    Ok(Json(rows))
}

async fn create_feed(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
    Json(data): Json<FeedCreate>,
) -> Result<(StatusCode, Json<FeedResponse>), AppError> {
    let campaign: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, profile_id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;
    let (_, campaign_profile_id) = campaign.ok_or_else(|| AppError::NotFound("Campaign not found".into()))?;

    let account: Option<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT profile_id, platform, account_handle, account_id
         FROM profile_accounts WHERE id = ?"
    ).bind(&data.profile_account_id).fetch_optional(&state.db).await?;
    let (account_profile_id, platform, _, account_id_opt) = account
        .ok_or_else(|| AppError::NotFound("Connected account not found".into()))?;

    if let Some(cpid) = &campaign_profile_id {
        if cpid != &account_profile_id {
            return Err(AppError::BadRequest(
                "Connected account belongs to a different profile than this campaign".into()
            ));
        }
    }

    let connectors = build_connectors(&state).await;
    let connector = connectors.get(platform.as_str())
        .ok_or_else(|| AppError::BadRequest(format!(
            "Platform '{}' has no connector configured.", platform
        )))?;
    if !connector.supports_feeds() {
        return Err(AppError::BadRequest(format!(
            "Platform '{}' does not support auto-feeds yet.", platform
        )));
    }

    // Resolve account_id if not yet cached on the account row.
    let account_id = match account_id_opt {
        Some(id) => id,
        None => {
            let handle: (String,) = sqlx::query_as(
                "SELECT account_handle FROM profile_accounts WHERE id = ?"
            ).bind(&data.profile_account_id).fetch_one(&state.db).await?;
            let resolved = connector.resolve_account_id(&handle.0).await
                .map_err(|e| AppError::BadRequest(format!("Could not resolve account: {}", e)))?;
            sqlx::query("UPDATE profile_accounts SET account_id = ? WHERE id = ?")
                .bind(&resolved).bind(&data.profile_account_id).execute(&state.db).await?;
            resolved
        }
    };

    // Seed cursor with the most recent post that's *outside* the grace window — anything posted
    // within the last SEED_GRACE_HOURS will then be discovered on the first tick (picks up posts
    // the user made just before setting up the feed).
    let seed = connector.list_new_posts(&account_id, &data.content_type, None).await
        .unwrap_or_default();
    let grace_cutoff = Utc::now() - Duration::hours(SEED_GRACE_HOURS);
    let cursor_post = seed.iter().find(|p|
        p.posted_at.map(|dt| dt <= grace_cutoff).unwrap_or(false)
    );
    let initial_cursor = cursor_post.map(|p| p.platform_post_id.clone());
    let initial_posted_at = cursor_post.and_then(|p| p.posted_at).map(|dt| dt.to_rfc3339());

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO campaign_feeds
         (id, campaign_id, profile_account_id, content_type,
          last_seen_post_id, last_seen_posted_at, last_checked_at, is_active)
         VALUES (?, ?, ?, ?, ?, ?, datetime('now'), 1)"
    )
        .bind(&id).bind(&campaign_id).bind(&data.profile_account_id).bind(&data.content_type)
        .bind(&initial_cursor).bind(&initial_posted_at)
        .execute(&state.db).await?;

    let row: FeedResponse = sqlx::query_as(
        "SELECT cf.id, cf.campaign_id, cf.profile_account_id,
                pa.platform, pa.account_handle, pa.account_id,
                pa.follower_count, pa.follower_count_at,
                cf.content_type, cf.last_seen_post_id, cf.last_checked_at,
                cf.last_error, cf.is_active, cf.created_at
         FROM campaign_feeds cf
         JOIN profile_accounts pa ON pa.id = cf.profile_account_id
         WHERE cf.id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(row)))
}

async fn delete_feed(
    State(state): State<Arc<AppState>>,
    Path(feed_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaign_feeds WHERE id = ?"
    ).bind(&feed_id).fetch_optional(&state.db).await?;
    if existing.is_none() {
        return Err(AppError::NotFound("Feed not found".into()));
    }
    sqlx::query("DELETE FROM campaign_feeds WHERE id = ?")
        .bind(&feed_id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
