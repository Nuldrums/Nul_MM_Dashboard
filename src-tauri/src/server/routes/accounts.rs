use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, routing::{get, delete}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::services::metric_collector::build_connectors;

#[derive(Deserialize)]
pub struct AccountCreate {
    pub platform: String,
    pub account_handle: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AccountResponse {
    pub id: String,
    pub profile_id: String,
    pub platform: String,
    pub account_handle: String,
    pub account_id: Option<String>,
    pub is_active: i32,
    pub has_oauth: i32,
    pub token_expires_at: Option<String>,
    pub created_at: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/profiles/{profile_id}/accounts", get(list_accounts).post(create_account))
        .route("/api/profile-accounts/{account_id}", delete(delete_account))
}

async fn list_accounts(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
) -> Result<Json<Vec<AccountResponse>>, AppError> {
    let rows: Vec<AccountResponse> = sqlx::query_as(
        "SELECT id, profile_id, platform, account_handle, account_id, is_active,
                CASE WHEN oauth_access_token IS NOT NULL THEN 1 ELSE 0 END AS has_oauth,
                token_expires_at, created_at
         FROM profile_accounts
         WHERE profile_id = ?
         ORDER BY platform, account_handle"
    ).bind(&profile_id).fetch_all(&state.db).await?;
    Ok(Json(rows))
}

async fn create_account(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
    Json(data): Json<AccountCreate>,
) -> Result<(StatusCode, Json<AccountResponse>), AppError> {
    let profile_exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profiles WHERE id = ?"
    ).bind(&profile_id).fetch_optional(&state.db).await?;
    if profile_exists.is_none() {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    let handle = data.account_handle.trim();
    if handle.is_empty() {
        return Err(AppError::BadRequest("Account handle is required".into()));
    }

    // Resolve handle -> stable account_id via the connector (validates the account exists).
    let connectors = build_connectors(&state).await;
    let connector = connectors.get(data.platform.as_str())
        .ok_or_else(|| AppError::BadRequest(format!(
            "Platform '{}' has no connector configured. Add credentials in Settings first.", data.platform
        )))?;
    if !connector.supports_feeds() {
        return Err(AppError::BadRequest(format!(
            "Platform '{}' does not support account connections yet.", data.platform
        )));
    }

    let account_id = connector.resolve_account_id(handle).await
        .map_err(|e| AppError::BadRequest(format!("Could not verify account '{}': {}", handle, e)))?;

    let id = uuid::Uuid::new_v4().to_string();
    let insert = sqlx::query(
        "INSERT INTO profile_accounts (id, profile_id, platform, account_handle, account_id)
         VALUES (?, ?, ?, ?, ?)"
    )
        .bind(&id).bind(&profile_id).bind(&data.platform).bind(handle).bind(&account_id)
        .execute(&state.db).await;

    if let Err(e) = insert {
        if e.to_string().contains("UNIQUE") {
            return Err(AppError::Conflict(format!(
                "{} account '{}' is already connected to this profile", data.platform, handle
            )));
        }
        return Err(AppError::Internal(format!("Failed to add account: {}", e)));
    }

    let row: AccountResponse = sqlx::query_as(
        "SELECT id, profile_id, platform, account_handle, account_id, is_active,
                CASE WHEN oauth_access_token IS NOT NULL THEN 1 ELSE 0 END AS has_oauth,
                token_expires_at, created_at
         FROM profile_accounts WHERE id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(row)))
}

async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profile_accounts WHERE id = ?"
    ).bind(&account_id).fetch_optional(&state.db).await?;
    if existing.is_none() {
        return Err(AppError::NotFound("Account not found".into()));
    }
    // ON DELETE CASCADE on campaign_feeds will clean up dependent feeds.
    sqlx::query("DELETE FROM profile_accounts WHERE id = ?")
        .bind(&account_id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
