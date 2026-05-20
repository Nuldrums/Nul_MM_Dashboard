use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use sqlx::SqlitePool;
use crate::server::connectors::{PlatformConnector, MetricResult, DiscoveredPost};

/// TikTok Display API connector.
///
/// Unlike YouTube/X which use app-level credentials, TikTok requires per-user OAuth.
/// Each profile_account row stores its own access_token / refresh_token; this connector
/// looks them up by `account_id` (which is the TikTok `open_id`).
pub struct TikTokConnector {
    client: reqwest::Client,
    client_key: String,
    client_secret: String,
    db: SqlitePool,
}

#[derive(Debug, Clone)]
struct AccountTokens {
    profile_account_id: String,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

impl TikTokConnector {
    pub fn new(client: reqwest::Client, client_key: String, client_secret: String, db: SqlitePool) -> Self {
        Self { client, client_key, client_secret, db }
    }

    fn configured(&self) -> bool {
        !self.client_key.is_empty() && !self.client_secret.is_empty()
    }

    /// Look up a profile_account by TikTok open_id, refreshing the token if expired.
    async fn tokens_for_open_id(&self, open_id: &str) -> anyhow::Result<AccountTokens> {
        let row: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, oauth_access_token, oauth_refresh_token, token_expires_at
             FROM profile_accounts WHERE platform = 'tiktok' AND account_id = ?"
        ).bind(open_id).fetch_optional(&self.db).await?;

        let (pa_id, access, refresh, expires) = row
            .ok_or_else(|| anyhow::anyhow!("No TikTok account connected for open_id {}", open_id))?;
        let access_token = access.ok_or_else(|| anyhow::anyhow!("TikTok account not OAuth-authorized yet"))?;
        let expires_at = expires.as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let mut tokens = AccountTokens {
            profile_account_id: pa_id,
            access_token,
            refresh_token: refresh,
            expires_at,
        };

        // Refresh proactively if within 60 seconds of expiry.
        if let Some(exp) = tokens.expires_at {
            if exp <= Utc::now() + Duration::seconds(60) {
                tracing::info!("TikTok token for open_id {} near expiry, refreshing", open_id);
                self.refresh_tokens(&mut tokens).await?;
            }
        }
        Ok(tokens)
    }

    /// Look up tokens for a post in our DB. Used by fetch_post_metrics where we only
    /// know the platform_post_id, not the account. Two paths:
    ///   1) Fast path — the post has profile_account_id set (auto-feed discoveries set this).
    ///   2) Fallback — manually-added posts have profile_account_id NULL; we find the
    ///      campaign's profile's TikTok account and backfill the link.
    async fn tokens_for_post(&self, platform_post_id: &str) -> anyhow::Result<AccountTokens> {
        // Fast path: post has a direct profile_account link
        let direct: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT pa.id, pa.oauth_access_token, pa.oauth_refresh_token, pa.token_expires_at
             FROM posts p
             JOIN profile_accounts pa ON pa.id = p.profile_account_id
             WHERE p.platform = 'tiktok' AND p.platform_post_id = ?
             LIMIT 1"
        ).bind(platform_post_id).fetch_optional(&self.db).await?;

        let row = match direct {
            Some(r) => r,
            None => {
                // Fallback: find a TikTok account for the post's campaign's profile.
                // Works for manually-added posts where profile_account_id is NULL.
                let fallback: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT pa.id, pa.oauth_access_token, pa.oauth_refresh_token, pa.token_expires_at
                     FROM posts p
                     JOIN campaigns c ON c.id = p.campaign_id
                     JOIN profile_accounts pa ON pa.profile_id = c.profile_id
                     WHERE p.platform = 'tiktok' AND p.platform_post_id = ?
                       AND pa.platform = 'tiktok' AND pa.is_active = 1
                       AND pa.oauth_access_token IS NOT NULL
                     LIMIT 1"
                ).bind(platform_post_id).fetch_optional(&self.db).await?;

                let row = fallback.ok_or_else(|| anyhow::anyhow!(
                    "TikTok post {} can't be tracked: no TikTok account is OAuth-authorized for this campaign's profile. Connect one in Settings.",
                    platform_post_id
                ))?;

                // Backfill the link so future fetches use the fast path.
                let _ = sqlx::query(
                    "UPDATE posts SET profile_account_id = ?
                     WHERE platform = 'tiktok' AND platform_post_id = ? AND profile_account_id IS NULL"
                ).bind(&row.0).bind(platform_post_id).execute(&self.db).await;
                tracing::info!("Backfilled profile_account_id={} for manually-added TikTok post {}", row.0, platform_post_id);

                row
            }
        };

        let (pa_id, access, refresh, expires) = row;
        let access_token = access.ok_or_else(|| anyhow::anyhow!("Account not OAuth-authorized"))?;
        let expires_at = expires.as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let mut tokens = AccountTokens {
            profile_account_id: pa_id,
            access_token,
            refresh_token: refresh,
            expires_at,
        };
        if let Some(exp) = tokens.expires_at {
            if exp <= Utc::now() + Duration::seconds(60) {
                self.refresh_tokens(&mut tokens).await?;
            }
        }
        Ok(tokens)
    }

    async fn refresh_tokens(&self, tokens: &mut AccountTokens) -> anyhow::Result<()> {
        let refresh_token = tokens.refresh_token.clone()
            .ok_or_else(|| anyhow::anyhow!("No refresh token available — re-authorize via OAuth"))?;

        let resp = self.client.post("https://open.tiktokapis.com/v2/oauth/token/")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("client_key", &self.client_key),
                ("client_secret", &self.client_secret),
                ("grant_type", &"refresh_token".to_string()),
                ("refresh_token", &refresh_token),
            ])
            .send().await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;
        if !status.is_success() || body["access_token"].as_str().is_none() {
            anyhow::bail!("TikTok token refresh failed: {} {}", status, body);
        }

        let new_access = body["access_token"].as_str().unwrap().to_string();
        let new_refresh = body["refresh_token"].as_str().map(|s| s.to_string()).or(tokens.refresh_token.clone());
        let expires_in = body["expires_in"].as_i64().unwrap_or(86400);
        let new_expires = Utc::now() + Duration::seconds(expires_in);

        sqlx::query(
            "UPDATE profile_accounts
             SET oauth_access_token = ?, oauth_refresh_token = ?, token_expires_at = ?
             WHERE id = ?"
        )
            .bind(&new_access).bind(&new_refresh).bind(new_expires.to_rfc3339())
            .bind(&tokens.profile_account_id)
            .execute(&self.db).await?;

        tokens.access_token = new_access;
        tokens.refresh_token = new_refresh;
        tokens.expires_at = Some(new_expires);
        Ok(())
    }
}

#[async_trait]
impl PlatformConnector for TikTokConnector {
    fn platform(&self) -> &str { "tiktok" }

    async fn validate_credentials(&self) -> bool {
        self.configured()
    }

    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult> {
        if !self.configured() {
            anyhow::bail!("TikTok app credentials not configured");
        }
        let tokens = self.tokens_for_post(platform_post_id).await?;

        let url = "https://open.tiktokapis.com/v2/video/query/?fields=id,view_count,like_count,comment_count,share_count";
        let body = serde_json::json!({
            "filters": { "video_ids": [platform_post_id] }
        });

        let resp = self.client.post(url)
            .bearer_auth(&tokens.access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            anyhow::bail!("TikTok video/query failed ({}): {}", status, data);
        }

        tracing::info!("TikTok /v2/video/query/ response for {}: {}", platform_post_id, data);

        let videos = data["data"]["videos"].as_array();
        let video = match videos.and_then(|v| v.first()) {
            Some(v) => v,
            None => anyhow::bail!(
                "TikTok video {} not found in response. Full response: {}",
                platform_post_id, data
            ),
        };

        Ok(MetricResult {
            views: video["view_count"].as_i64().unwrap_or(0),
            likes: video["like_count"].as_i64().unwrap_or(0),
            comments: video["comment_count"].as_i64().unwrap_or(0),
            shares: video["share_count"].as_i64().unwrap_or(0),
            fetched_via: "api".into(),
            ..Default::default()
        })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // /video/<numeric_id> or /@handle/video/<id>
        if let Some(pos) = url.find("/video/") {
            let rest = &url[pos + 7..];
            let end = rest.find('?').or_else(|| rest.find('/')).unwrap_or(rest.len());
            let id = &rest[..end];
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
        None
    }

    fn is_api_trackable(&self) -> bool { self.configured() }

    fn supports_feeds(&self) -> bool { self.configured() }

    async fn resolve_account_id(&self, _handle: &str) -> anyhow::Result<String> {
        // TikTok's open_id isn't lookup-able from a handle — it's only known after OAuth.
        // The OAuth callback writes the resolved open_id directly to profile_accounts.account_id.
        anyhow::bail!(
            "TikTok accounts must be connected via OAuth. Use the 'Connect TikTok' button instead of entering a handle."
        )
    }

    async fn fetch_follower_count(&self, account_id: &str) -> anyhow::Result<Option<i64>> {
        if !self.configured() {
            return Ok(None);
        }
        let tokens = self.tokens_for_open_id(account_id).await?;
        let url = "https://open.tiktokapis.com/v2/user/info/?fields=open_id,follower_count";
        let resp = self.client.get(url)
            .bearer_auth(&tokens.access_token)
            .send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            anyhow::bail!("TikTok user.info failed ({}): {}", status, data);
        }
        Ok(data["data"]["user"]["follower_count"].as_i64())
    }

    async fn list_new_posts(
        &self,
        account_id: &str,
        _content_type: &str,
        since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        if !self.configured() {
            anyhow::bail!("TikTok app credentials not configured");
        }
        let tokens = self.tokens_for_open_id(account_id).await?;

        let url = "https://open.tiktokapis.com/v2/video/list/?fields=id,title,create_time,share_url,duration";
        let body = serde_json::json!({ "max_count": 20 });

        let resp = self.client.post(url)
            .bearer_auth(&tokens.access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            anyhow::bail!("TikTok video/list failed ({}): {}", status, data);
        }

        let videos = data["data"]["videos"].as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        for v in &videos {
            let id = v["id"].as_str().unwrap_or("");
            if id.is_empty() { continue; }
            if Some(id) == since_post_id { break; }

            let title = v["title"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());
            let posted_at = v["create_time"].as_i64()
                .and_then(|ts| DateTime::from_timestamp(ts, 0));
            let share_url = v["share_url"].as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("https://www.tiktok.com/video/{}", id));

            out.push(DiscoveredPost {
                platform_post_id: id.to_string(),
                url: share_url,
                title,
                posted_at,
                post_type: "video_short".into(),
            });
        }
        Ok(out)
    }
}
