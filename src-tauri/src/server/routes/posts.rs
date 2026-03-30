use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, routing::{get, put, delete}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::db::models::{PostRow, deserialize_tags_from_input, serialize_tags_to_json_string};

#[derive(Deserialize)]
pub struct PostCreate {
    pub platform: String,
    pub post_type: String,
    pub platform_post_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_preview: Option<String>,
    pub target_community: Option<String>,
    pub posted_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub is_api_tracked: Option<i32>,
}

#[derive(Deserialize)]
pub struct PostUpdate {
    pub platform: Option<String>,
    pub post_type: Option<String>,
    pub platform_post_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_preview: Option<String>,
    pub target_community: Option<String>,
    pub posted_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub is_api_tracked: Option<i32>,
}

#[derive(Serialize)]
pub struct PostResponse {
    pub id: String,
    pub campaign_id: String,
    pub platform: String,
    pub post_type: String,
    pub platform_post_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_preview: Option<String>,
    pub target_community: Option<String>,
    pub posted_at: Option<String>,
    #[serde(serialize_with = "serialize_tags_to_json_string")]
    pub tags: Option<String>,
    pub is_api_tracked: i32,
    pub created_at: Option<String>,
}

impl From<PostRow> for PostResponse {
    fn from(r: PostRow) -> Self {
        Self {
            id: r.id,
            campaign_id: r.campaign_id,
            platform: r.platform,
            post_type: r.post_type,
            platform_post_id: r.platform_post_id,
            url: r.url,
            title: r.title,
            body_preview: r.body_preview,
            target_community: r.target_community,
            posted_at: r.posted_at.map(|dt| dt.to_string()),
            tags: r.tags,
            is_api_tracked: r.is_api_tracked,
            created_at: r.created_at.map(|dt| dt.to_string()),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/campaigns/{campaign_id}/posts", get(list_posts).post(create_post))
        .route("/api/posts/{post_id}", put(update_post).delete(delete_post))
}

async fn list_posts(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
) -> Result<Json<Vec<PostResponse>>, AppError> {
    // Verify campaign exists
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Campaign not found".into()));
    }

    let rows = sqlx::query_as::<_, PostRow>(
        "SELECT id, campaign_id, platform, post_type, platform_post_id, url, title,
                body_preview, target_community, posted_at, tags, is_api_tracked, created_at
         FROM posts WHERE campaign_id = ? ORDER BY created_at DESC"
    ).bind(&campaign_id).fetch_all(&state.db).await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_post(
    State(state): State<Arc<AppState>>,
    Path(campaign_id): Path<String>,
    Json(data): Json<PostCreate>,
) -> Result<(StatusCode, Json<PostResponse>), AppError> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE id = ?"
    ).bind(&campaign_id).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Campaign not found".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let is_api_tracked = data.is_api_tracked.unwrap_or(0);

    sqlx::query(
        "INSERT INTO posts (id, campaign_id, platform, post_type, platform_post_id, url, title,
         body_preview, target_community, posted_at, tags, is_api_tracked)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(&id).bind(&campaign_id).bind(&data.platform).bind(&data.post_type)
        .bind(&data.platform_post_id).bind(&data.url).bind(&data.title)
        .bind(&data.body_preview).bind(&data.target_community).bind(&data.posted_at)
        .bind(&data.tags).bind(is_api_tracked)
        .execute(&state.db).await?;

    let row = sqlx::query_as::<_, PostRow>(
        "SELECT id, campaign_id, platform, post_type, platform_post_id, url, title,
                body_preview, target_community, posted_at, tags, is_api_tracked, created_at
         FROM posts WHERE id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn update_post(
    State(state): State<Arc<AppState>>,
    Path(post_id): Path<String>,
    Json(data): Json<PostUpdate>,
) -> Result<Json<PostResponse>, AppError> {
    let existing = sqlx::query_as::<_, PostRow>(
        "SELECT id, campaign_id, platform, post_type, platform_post_id, url, title,
                body_preview, target_community, posted_at, tags, is_api_tracked, created_at
         FROM posts WHERE id = ?"
    ).bind(&post_id).fetch_optional(&state.db).await?;

    let row = existing.ok_or_else(|| AppError::NotFound("Post not found".into()))?;

    let platform = data.platform.unwrap_or(row.platform);
    let post_type = data.post_type.unwrap_or(row.post_type);
    let platform_post_id = data.platform_post_id.or(row.platform_post_id);
    let url = data.url.or(row.url);
    let title = data.title.or(row.title);
    let body_preview = data.body_preview.or(row.body_preview);
    let target_community = data.target_community.or(row.target_community);
    let posted_at = data.posted_at.or(row.posted_at.map(|dt| dt.to_string()));
    let tags = data.tags.or(row.tags);
    let is_api_tracked = data.is_api_tracked.unwrap_or(row.is_api_tracked);

    sqlx::query(
        "UPDATE posts SET platform=?, post_type=?, platform_post_id=?, url=?, title=?,
         body_preview=?, target_community=?, posted_at=?, tags=?, is_api_tracked=? WHERE id=?"
    )
        .bind(&platform).bind(&post_type).bind(&platform_post_id)
        .bind(&url).bind(&title).bind(&body_preview)
        .bind(&target_community).bind(&posted_at).bind(&tags)
        .bind(is_api_tracked).bind(&post_id)
        .execute(&state.db).await?;

    let updated = sqlx::query_as::<_, PostRow>(
        "SELECT id, campaign_id, platform, post_type, platform_post_id, url, title,
                body_preview, target_community, posted_at, tags, is_api_tracked, created_at
         FROM posts WHERE id = ?"
    ).bind(&post_id).fetch_one(&state.db).await?;

    Ok(Json(updated.into()))
}

async fn delete_post(
    State(state): State<Arc<AppState>>,
    Path(post_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM posts WHERE id = ?"
    ).bind(&post_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Post not found".into()));
    }

    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(&post_id).execute(&state.db).await?;

    Ok(StatusCode::NO_CONTENT)
}
