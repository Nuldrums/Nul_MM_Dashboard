use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, routing::{get, put, delete}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::db::models::ProfileRow;

#[derive(Deserialize)]
pub struct ProfileCreate {
    pub name: String,
    pub description: Option<String>,
    pub avatar_color: Option<String>,
}

#[derive(Deserialize)]
pub struct ProfileUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_color: Option<String>,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_color: Option<String>,
    pub created_at: Option<String>,
}

impl From<ProfileRow> for ProfileResponse {
    fn from(r: ProfileRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            avatar_color: r.avatar_color,
            created_at: r.created_at.map(|dt| dt.to_string()),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/profiles", get(list_profiles).post(create_profile))
        .route("/api/profiles/{profile_id}", put(update_profile).delete(delete_profile))
}

async fn list_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProfileResponse>>, AppError> {
    tracing::info!("Listing profiles");
    let rows = sqlx::query_as::<_, ProfileRow>(
        "SELECT id, name, description, avatar_color, created_at FROM profiles ORDER BY created_at ASC"
    ).fetch_all(&state.db).await?;
    tracing::info!("Found {} profiles", rows.len());
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(data): Json<ProfileCreate>,
) -> Result<(StatusCode, Json<ProfileResponse>), AppError> {
    tracing::info!("Creating profile: name={:?}, color={:?}", data.name, data.avatar_color);
    // Check for duplicate name
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profiles WHERE name = ?"
    ).bind(&data.name).fetch_optional(&state.db).await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Profile with this name already exists".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let color = data.avatar_color.unwrap_or_else(|| "#E8845C".into());

    sqlx::query(
        "INSERT INTO profiles (id, name, description, avatar_color) VALUES (?, ?, ?, ?)"
    )
        .bind(&id)
        .bind(&data.name)
        .bind(&data.description)
        .bind(&color)
        .execute(&state.db).await?;

    let row = sqlx::query_as::<_, ProfileRow>(
        "SELECT id, name, description, avatar_color, created_at FROM profiles WHERE id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    tracing::info!("Profile created: id={}", id);
    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
    Json(data): Json<ProfileUpdate>,
) -> Result<Json<ProfileResponse>, AppError> {
    let existing = sqlx::query_as::<_, ProfileRow>(
        "SELECT id, name, description, avatar_color, created_at FROM profiles WHERE id = ?"
    ).bind(&profile_id).fetch_optional(&state.db).await?;

    let row = existing.ok_or_else(|| AppError::NotFound("Profile not found".into()))?;

    // Check name uniqueness if changing name
    if let Some(ref new_name) = data.name {
        if new_name != &row.name {
            let dup: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM profiles WHERE name = ?"
            ).bind(new_name).fetch_optional(&state.db).await?;
            if dup.is_some() {
                return Err(AppError::Conflict("Profile with this name already exists".into()));
            }
        }
    }

    let name = data.name.unwrap_or(row.name);
    let description = data.description.or(row.description);
    let avatar_color = data.avatar_color.or(row.avatar_color);

    sqlx::query(
        "UPDATE profiles SET name = ?, description = ?, avatar_color = ? WHERE id = ?"
    )
        .bind(&name)
        .bind(&description)
        .bind(&avatar_color)
        .bind(&profile_id)
        .execute(&state.db).await?;

    let updated = sqlx::query_as::<_, ProfileRow>(
        "SELECT id, name, description, avatar_color, created_at FROM profiles WHERE id = ?"
    ).bind(&profile_id).fetch_one(&state.db).await?;

    Ok(Json(updated.into()))
}

async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profiles WHERE id = ?"
    ).bind(&profile_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    let campaign_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM campaigns WHERE profile_id = ? AND status != 'archived'"
    ).bind(&profile_id).fetch_one(&state.db).await?;

    if campaign_count.0 > 0 {
        return Err(AppError::Conflict(format!(
            "Cannot delete profile with {} active campaign(s). Archive or delete them first.",
            campaign_count.0
        )));
    }

    // Clean up archived campaigns' posts and metrics, then the campaigns themselves
    let archived_campaigns: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE profile_id = ? AND status = 'archived'"
    ).bind(&profile_id).fetch_all(&state.db).await?;

    for (cid,) in &archived_campaigns {
        sqlx::query("DELETE FROM metric_snapshots WHERE post_id IN (SELECT id FROM posts WHERE campaign_id = ?)")
            .bind(cid).execute(&state.db).await?;
        sqlx::query("DELETE FROM posts WHERE campaign_id = ?")
            .bind(cid).execute(&state.db).await?;
        sqlx::query("DELETE FROM ai_analyses WHERE campaign_id = ?")
            .bind(cid).execute(&state.db).await?;
    }
    sqlx::query("DELETE FROM campaigns WHERE profile_id = ? AND status = 'archived'")
        .bind(&profile_id).execute(&state.db).await?;

    // Clean up products belonging to this profile
    sqlx::query("DELETE FROM products WHERE profile_id = ?")
        .bind(&profile_id).execute(&state.db).await?;

    sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(&profile_id).execute(&state.db).await?;

    Ok(Json(serde_json::json!({"message": "Profile deleted", "id": profile_id})))
}
