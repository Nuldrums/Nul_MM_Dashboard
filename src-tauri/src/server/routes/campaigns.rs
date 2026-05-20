use std::sync::Arc;
use axum::{extract::{Path, Query, State}, http::StatusCode, routing::{get, put, delete}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::db::models::{CampaignRow, serialize_tags_to_json_string, deserialize_tags_from_input, parse_json_column};

#[derive(Deserialize)]
pub struct ProfileIdFilter {
    pub profile_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CampaignCreate {
    pub product_id: String,
    pub name: String,
    pub status: Option<String>,
    pub goal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub target_audience: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CampaignUpdate {
    pub product_id: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub goal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub target_audience: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Serialize)]
pub struct CampaignResponse {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub status: Option<String>,
    pub goal: Option<String>,
    #[serde(serialize_with = "serialize_tags_to_json_string")]
    pub target_audience: Option<String>,
    #[serde(serialize_with = "serialize_tags_to_json_string")]
    pub tags: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
    pub profile_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub post_count: i64,
    pub total_likes: i64,
    pub total_comments: i64,
    pub total_views: i64,
    pub platforms: Vec<String>,
}

impl CampaignResponse {
    fn from_row(
        r: CampaignRow,
        post_count: i64,
        total_likes: i64,
        total_comments: i64,
        total_views: i64,
        platforms: Vec<String>,
    ) -> Self {
        Self {
            id: r.id,
            product_id: r.product_id,
            name: r.name,
            status: r.status,
            goal: r.goal,
            target_audience: r.target_audience,
            tags: r.tags,
            start_date: r.start_date,
            end_date: r.end_date,
            notes: r.notes,
            profile_id: r.profile_id,
            created_at: r.created_at.map(|dt| dt.to_string()),
            updated_at: r.updated_at.map(|dt| dt.to_string()),
            post_count,
            total_likes,
            total_comments,
            total_views,
            platforms,
        }
    }
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub permanent: Option<bool>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/campaigns", get(list_campaigns).post(create_campaign))
        .route("/api/campaigns/{campaign_id}", get(get_campaign).put(update_campaign).delete(delete_campaign))
}

async fn list_campaigns(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<CampaignResponse>>, AppError> {
    let campaigns = if let Some(pid) = params.profile_id {
        sqlx::query_as::<_, CampaignRow>(
            "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                    start_date, end_date, notes, created_at, updated_at
             FROM campaigns WHERE profile_id = ? ORDER BY created_at DESC"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as::<_, CampaignRow>(
            "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                    start_date, end_date, notes, created_at, updated_at
             FROM campaigns ORDER BY created_at DESC"
        ).fetch_all(&state.db).await?
    };

    let mut response = Vec::new();
    for campaign in campaigns {
        let cid = campaign.id.clone();

        let (post_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM posts WHERE campaign_id = ?"
        ).bind(&cid).fetch_one(&state.db).await?;

        // Sum the latest snapshot per post (snapshots are cumulative point-in-time totals)
        let metrics: (i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(latest.likes), 0), COALESCE(SUM(latest.comments), 0), COALESCE(SUM(latest.views), 0)
             FROM posts p
             JOIN (
                 SELECT post_id, views, likes, comments,
                        ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY snapshot_date DESC) AS rn
                 FROM metric_snapshots
             ) latest ON latest.post_id = p.id AND latest.rn = 1
             WHERE p.campaign_id = ?"
        ).bind(&cid).fetch_one(&state.db).await?;

        let platform_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT platform FROM posts WHERE campaign_id = ?"
        ).bind(&cid).fetch_all(&state.db).await?;
        let platforms = platform_rows.into_iter().map(|(p,)| p).collect();

        response.push(CampaignResponse::from_row(campaign, post_count, metrics.0, metrics.1, metrics.2, platforms));
    }

    Ok(Json(response))
}

async fn get_campaign(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let campaign = sqlx::query_as::<_, CampaignRow>(
        "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                start_date, end_date, notes, created_at, updated_at
         FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;

    let campaign = campaign.ok_or_else(|| AppError::NotFound("Campaign not found".into()))?;

    // Fetch posts with their latest snapshot's metrics (snapshots are cumulative totals,
    // so summing across dates would double-count). Falls back to 0 when no snapshot exists.
    let posts: Vec<(String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, i32, Option<String>, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT p.id, p.platform, p.post_type, p.title, p.url, p.target_community, p.posted_at, p.is_api_tracked, p.created_at,
                COALESCE(latest.views, 0)    AS views,
                COALESCE(latest.likes, 0)    AS likes,
                COALESCE(latest.comments, 0) AS comments,
                COALESCE(latest.shares, 0)   AS shares
         FROM posts p
         LEFT JOIN (
             SELECT post_id, views, likes, comments, shares,
                    ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY snapshot_date DESC) AS rn
             FROM metric_snapshots
         ) latest ON latest.post_id = p.id AND latest.rn = 1
         WHERE p.campaign_id = ?
         ORDER BY p.created_at DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    let posts_data: Vec<serde_json::Value> = posts.into_iter().map(|p| {
        let views = p.9; let likes = p.10; let comments = p.11; let shares = p.12;
        serde_json::json!({
            "id": p.0,
            "platform": p.1,
            "post_type": p.2,
            "title": p.3,
            "url": p.4,
            "target_community": p.5,
            "posted_at": p.6,
            "is_api_tracked": p.7,
            "created_at": p.8,
            "views": views,
            "likes": likes,
            "comments": comments,
            "shares": shares,
            "engagement": likes + comments + shares,
        })
    }).collect();

    let post_count = posts_data.len() as i64;

    Ok(Json(serde_json::json!({
        "id": campaign.id,
        "product_id": campaign.product_id,
        "name": campaign.name,
        "status": campaign.status,
        "goal": campaign.goal,
        "target_audience": parse_json_column(&campaign.target_audience),
        "tags": parse_json_column(&campaign.tags),
        "start_date": campaign.start_date,
        "end_date": campaign.end_date,
        "notes": campaign.notes,
        "profile_id": campaign.profile_id,
        "created_at": campaign.created_at.map(|dt| dt.to_string()),
        "updated_at": campaign.updated_at.map(|dt| dt.to_string()),
        "post_count": post_count,
        "posts": posts_data,
    })))
}

async fn create_campaign(
    State(state): State<Arc<AppState>>,
    Json(data): Json<CampaignCreate>,
) -> Result<(StatusCode, Json<CampaignResponse>), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let status = data.status.unwrap_or_else(|| "active".into());

    sqlx::query(
        "INSERT INTO campaigns (id, product_id, profile_id, name, status, goal, target_audience, tags, start_date, end_date, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(&id).bind(&data.product_id).bind(&data.profile_id)
        .bind(&data.name).bind(&status).bind(&data.goal)
        .bind(&data.target_audience).bind(&data.tags)
        .bind(&data.start_date).bind(&data.end_date)
        .bind(&data.notes)
        .execute(&state.db).await?;

    let row = sqlx::query_as::<_, CampaignRow>(
        "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                start_date, end_date, notes, created_at, updated_at
         FROM campaigns WHERE id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(CampaignResponse::from_row(row, 0, 0, 0, 0, Vec::new()))))
}

async fn update_campaign(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
    Json(data): Json<CampaignUpdate>,
) -> Result<Json<CampaignResponse>, AppError> {
    let existing = sqlx::query_as::<_, CampaignRow>(
        "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                start_date, end_date, notes, created_at, updated_at
         FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;

    let row = existing.ok_or_else(|| AppError::NotFound("Campaign not found".into()))?;

    let product_id = data.product_id.unwrap_or(row.product_id);
    let name = data.name.unwrap_or(row.name);
    let status = data.status.or(row.status);
    let goal = data.goal.or(row.goal);
    let target_audience = data.target_audience.or(row.target_audience);
    let tags = data.tags.or(row.tags);
    let start_date = data.start_date.or(row.start_date);
    let end_date = data.end_date.or(row.end_date);
    let notes = data.notes.or(row.notes);
    let profile_id = data.profile_id.or(row.profile_id);

    sqlx::query(
        "UPDATE campaigns SET product_id=?, name=?, status=?, goal=?, target_audience=?, tags=?,
         start_date=?, end_date=?, notes=?, profile_id=?, updated_at=datetime('now') WHERE id=?"
    )
        .bind(&product_id).bind(&name).bind(&status).bind(&goal)
        .bind(&target_audience).bind(&tags).bind(&start_date).bind(&end_date)
        .bind(&notes).bind(&profile_id).bind(&campaign_id)
        .execute(&state.db).await?;

    let updated = sqlx::query_as::<_, CampaignRow>(
        "SELECT id, product_id, profile_id, name, status, goal, target_audience, tags,
                start_date, end_date, notes, created_at, updated_at
         FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_one(&state.db).await?;

    Ok(Json(CampaignResponse::from_row(updated, 0, 0, 0, 0, Vec::new())))
}

async fn delete_campaign(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
    Query(params): Query<DeleteParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Campaign not found".into()));
    }

    if params.permanent.unwrap_or(false) {
        // Hard delete: cascade remove posts, metrics, analyses, then campaign
        sqlx::query("DELETE FROM metric_snapshots WHERE post_id IN (SELECT id FROM posts WHERE campaign_id = ?)")
            .bind(&campaign_id).execute(&state.db).await?;
        sqlx::query("DELETE FROM posts WHERE campaign_id = ?")
            .bind(&campaign_id).execute(&state.db).await?;
        sqlx::query("DELETE FROM ai_analyses WHERE campaign_id = ?")
            .bind(&campaign_id).execute(&state.db).await?;
        sqlx::query("DELETE FROM campaigns WHERE id = ?")
            .bind(&campaign_id).execute(&state.db).await?;

        Ok(Json(serde_json::json!({"message": "Campaign permanently deleted", "id": campaign_id})))
    } else {
        // Soft delete: archive
        sqlx::query("UPDATE campaigns SET status = 'archived', updated_at = datetime('now') WHERE id = ?")
            .bind(&campaign_id).execute(&state.db).await?;

        Ok(Json(serde_json::json!({"message": "Campaign archived", "id": campaign_id})))
    }
}
