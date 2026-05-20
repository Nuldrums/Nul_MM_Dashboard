use std::sync::Arc;
use axum::{extract::{Query, State}, http::StatusCode, response::Html, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use crate::server::{AppState, OAuthPendingState, error::AppError};

const TIKTOK_REDIRECT_PATH: &str = "/api/oauth/tiktok/callback";
const TIKTOK_AUTH_URL: &str = "https://www.tiktok.com/v2/auth/authorize/";
const TIKTOK_TOKEN_URL: &str = "https://open.tiktokapis.com/v2/oauth/token/";
const TIKTOK_USER_INFO_URL: &str = "https://open.tiktokapis.com/v2/user/info/?fields=open_id,union_id,avatar_url,display_name,username";

#[derive(Deserialize)]
pub struct StartParams {
    pub profile_id: String,
}

#[derive(Serialize)]
pub struct StartResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/oauth/tiktok/start", get(tiktok_start))
        .route("/api/oauth/tiktok/exchange", post(tiktok_exchange))
        .route(TIKTOK_REDIRECT_PATH, get(tiktok_callback))
}

/// Where TikTok should send the user after they authorize. In production this is the public
/// relay on nuldrums.world which 302-redirects to a meem:// deep link. Override via the
/// MEEM_TIKTOK_REDIRECT_URI env var for local dev or alternate setups (e.g. cloudflared tunnel).
fn redirect_uri(_state: &Arc<AppState>) -> String {
    std::env::var("MEEM_TIKTOK_REDIRECT_URI")
        .unwrap_or_else(|_| "https://nuldrums.world/api/oauth/tiktok-callback".to_string())
}

async fn tiktok_credentials(state: &Arc<AppState>) -> Result<(String, String), AppError> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT credentials FROM platform_configs WHERE platform = 'tiktok'"
    ).fetch_optional(&state.db).await?;
    let creds_str = row.and_then(|(c,)| c).unwrap_or_default();
    let creds: serde_json::Value = serde_json::from_str(&creds_str).unwrap_or(serde_json::json!({}));
    let key = creds["client_key"].as_str().unwrap_or("").to_string();
    let secret = creds["client_secret"].as_str().unwrap_or("").to_string();
    if key.is_empty() || secret.is_empty() {
        return Err(AppError::BadRequest(
            "TikTok client_key/client_secret not configured. Add them in Settings → TikTok.".into()
        ));
    }
    Ok((key, secret))
}

async fn tiktok_start(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StartParams>,
) -> Result<Json<StartResponse>, AppError> {
    let profile_exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profiles WHERE id = ?"
    ).bind(&params.profile_id).fetch_optional(&state.db).await?;
    if profile_exists.is_none() {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    let (client_key, _) = tiktok_credentials(&state).await?;

    let csrf = uuid::Uuid::new_v4().to_string().replace('-', "");
    {
        let mut pending = state.oauth_pending.lock().await;
        // Clean up entries older than 10 minutes.
        let cutoff = Utc::now() - Duration::minutes(10);
        pending.retain(|_, v| v.created_at > cutoff);

        pending.insert(csrf.clone(), OAuthPendingState {
            profile_id: params.profile_id.clone(),
            platform: "tiktok".into(),
            created_at: Utc::now(),
        });
    }

    let redirect = redirect_uri(&state);
    // user.info.stats gives us follower_count + video_count; user.info.profile gives bio/verified.
    // Both must be enabled in the TikTok app's Scopes config too — they're harmless if missing
    // (TikTok just won't grant them) but extra useful if present.
    let scope = "user.info.basic,user.info.profile,user.info.stats,video.list";
    let auth_url = format!(
        "{}?client_key={}&response_type=code&scope={}&redirect_uri={}&state={}",
        TIKTOK_AUTH_URL,
        urlencoding::encode(&client_key),
        urlencoding::encode(scope),
        urlencoding::encode(&redirect),
        csrf,
    );

    Ok(Json(StartResponse { auth_url, state: csrf }))
}

/// Browser-facing callback. Used when the redirect_uri points directly at this backend
/// (dev / tunnel mode). Renders HTML success/error pages.
async fn tiktok_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
) -> Html<String> {
    if let Some(err) = &params.error {
        let desc = params.error_description.as_deref().unwrap_or("");
        return Html(error_page(&format!("TikTok returned an error: {} {}", err, desc)));
    }
    let code = match params.code {
        Some(c) if !c.is_empty() => c,
        _ => return Html(error_page("Missing authorization code")),
    };
    let csrf = match params.state {
        Some(s) if !s.is_empty() => s,
        _ => return Html(error_page("Missing state parameter")),
    };

    match complete_tiktok_oauth(&state, &code, &csrf).await {
        Ok(display_name) => Html(success_page(&display_name)),
        Err(msg) => Html(error_page(&msg)),
    }
}

#[derive(Deserialize)]
pub struct ExchangeBody {
    pub code: String,
    pub state: String,
}

#[derive(Serialize)]
pub struct ExchangeResponse {
    pub display_name: String,
    pub open_id: String,
}

/// JSON-returning endpoint used by the Tauri deep-link handler. The website relay
/// (nuldrums.world) redirects to meem://oauth/tiktok/callback?code=...&state=...,
/// the frontend parses those params and POSTs them here to complete the exchange.
async fn tiktok_exchange(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ExchangeBody>,
) -> Result<(StatusCode, Json<ExchangeResponse>), AppError> {
    if body.code.is_empty() {
        return Err(AppError::BadRequest("Missing code".into()));
    }
    if body.state.is_empty() {
        return Err(AppError::BadRequest("Missing state".into()));
    }
    match complete_tiktok_oauth(&state, &body.code, &body.state).await {
        Ok(display_name) => Ok((StatusCode::OK, Json(ExchangeResponse {
            display_name,
            open_id: String::new(),
        }))),
        Err(msg) => Err(AppError::BadRequest(msg)),
    }
}

/// Shared OAuth exchange logic. Returns the connected account's display name on success,
/// or a user-facing error message on failure. Side effects: removes the CSRF state from
/// memory, upserts a profile_accounts row with the new tokens.
async fn complete_tiktok_oauth(
    state: &Arc<AppState>,
    code: &str,
    csrf: &str,
) -> Result<String, String> {
    let pending = {
        let mut map = state.oauth_pending.lock().await;
        map.remove(csrf)
    };
    let pending = pending.ok_or_else(||
        "Unknown or expired state. Please start the connection again.".to_string()
    )?;

    let (client_key, client_secret) = tiktok_credentials(state).await
        .map_err(e_to_string)?;

    let redirect = redirect_uri(state);
    let token_resp = state.http_client.post(TIKTOK_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_key", &client_key),
            ("client_secret", &client_secret),
            ("code", &code.to_string()),
            ("grant_type", &"authorization_code".to_string()),
            ("redirect_uri", &redirect),
        ])
        .send().await
        .map_err(|e| format!("Token request failed: {}", e))?;

    let token_data: serde_json::Value = token_resp.json().await
        .map_err(|e| format!("Token response parse failed: {}", e))?;

    let access_token = token_data["access_token"].as_str()
        .ok_or_else(|| format!("Token exchange failed: {}", token_data))?
        .to_string();
    let refresh_token = token_data["refresh_token"].as_str().map(|s| s.to_string());
    let expires_in = token_data["expires_in"].as_i64().unwrap_or(86400);
    let expires_at = Utc::now() + Duration::seconds(expires_in);
    let open_id_from_token = token_data["open_id"].as_str().map(|s| s.to_string());

    let info_resp = state.http_client.get(TIKTOK_USER_INFO_URL)
        .bearer_auth(&access_token)
        .send().await;
    let (open_id, display_name) = match info_resp {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(v) => {
                let user = &v["data"]["user"];
                let oid = user["open_id"].as_str().map(|s| s.to_string())
                    .or(open_id_from_token.clone())
                    .unwrap_or_default();
                let name = user["username"].as_str()
                    .or_else(|| user["display_name"].as_str())
                    .unwrap_or("tiktok-user")
                    .to_string();
                (oid, name)
            }
            Err(_) => (open_id_from_token.unwrap_or_default(), "tiktok-user".into()),
        },
        Err(_) => (open_id_from_token.unwrap_or_default(), "tiktok-user".into()),
    };

    if open_id.is_empty() {
        return Err("TikTok did not return an open_id".into());
    }

    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM profile_accounts WHERE profile_id = ? AND platform = 'tiktok' AND account_id = ?"
    ).bind(&pending.profile_id).bind(&open_id).fetch_optional(&state.db).await
        .map_err(|e| format!("DB lookup failed: {}", e))?;

    let upsert = if let Some((id,)) = existing {
        sqlx::query(
            "UPDATE profile_accounts
             SET oauth_access_token = ?, oauth_refresh_token = ?, token_expires_at = ?,
                 account_handle = ?, is_active = 1
             WHERE id = ?"
        )
            .bind(&access_token).bind(&refresh_token).bind(expires_at.to_rfc3339())
            .bind(&display_name).bind(&id)
            .execute(&state.db).await
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO profile_accounts
             (id, profile_id, platform, account_handle, account_id,
              oauth_access_token, oauth_refresh_token, token_expires_at, is_active)
             VALUES (?, ?, 'tiktok', ?, ?, ?, ?, ?, 1)"
        )
            .bind(&new_id).bind(&pending.profile_id)
            .bind(&display_name).bind(&open_id)
            .bind(&access_token).bind(&refresh_token).bind(expires_at.to_rfc3339())
            .execute(&state.db).await
    };

    upsert.map_err(|e| format!("Failed to save account: {}", e))?;
    Ok(display_name)
}

fn e_to_string(err: AppError) -> String {
    match err {
        AppError::NotFound(s) | AppError::Conflict(s) | AppError::BadRequest(s) | AppError::Internal(s) => s,
    }
}

fn success_page(handle: &str) -> String {
    format!(
        r#"<!doctype html><meta charset="utf-8"><title>TikTok Connected</title>
<style>body{{font-family:system-ui,sans-serif;background:#0f1117;color:#e5e7eb;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}}
.card{{background:#1a1d29;border:1px solid #2d3142;border-radius:12px;padding:32px;max-width:420px;text-align:center}}
h1{{margin:0 0 8px;font-size:1.25rem}}p{{color:#9ca3af;margin:8px 0 0}}</style>
<div class="card"><h1>Connected to TikTok</h1>
<p>@{} is now linked. You can close this tab and return to the dashboard.</p></div>"#,
        html_escape(handle),
    )
}

fn error_page(msg: &str) -> String {
    format!(
        r#"<!doctype html><meta charset="utf-8"><title>TikTok Connection Failed</title>
<style>body{{font-family:system-ui,sans-serif;background:#0f1117;color:#e5e7eb;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}}
.card{{background:#1a1d29;border:1px solid #b45309;border-radius:12px;padding:32px;max-width:520px}}
h1{{margin:0 0 8px;font-size:1.25rem;color:#f59e0b}}p{{color:#9ca3af;margin:8px 0 0;line-height:1.5}}</style>
<div class="card"><h1>Connection failed</h1><p>{}</p>
<p>Close this tab, fix the issue, then try again from the dashboard.</p></div>"#,
        html_escape(msg),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
