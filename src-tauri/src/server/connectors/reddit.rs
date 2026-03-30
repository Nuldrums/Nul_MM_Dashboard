use async_trait::async_trait;
use tokio::sync::RwLock;
use crate::server::connectors::{PlatformConnector, MetricResult};

pub struct RedditConnector {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
    token: RwLock<Option<TokenData>>,
}

struct TokenData {
    access_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl RedditConnector {
    pub fn new(
        client: reqwest::Client,
        client_id: String,
        client_secret: String,
        username: String,
        password: String,
    ) -> Self {
        Self {
            client, client_id, client_secret, username, password,
            token: RwLock::new(None),
        }
    }

    async fn get_token(&self) -> anyhow::Result<String> {
        // Check cached token
        {
            let guard = self.token.read().await;
            if let Some(ref t) = *guard {
                if t.expires_at > chrono::Utc::now() {
                    return Ok(t.access_token.clone());
                }
            }
        }

        // Fetch new token
        let resp = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .header("User-Agent", "MeemMarketing/0.1")
            .form(&[
                ("grant_type", "password"),
                ("username", &self.username),
                ("password", &self.password),
            ])
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let access_token = data["access_token"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No access_token in Reddit response"))?
            .to_string();
        let expires_in = data["expires_in"].as_i64().unwrap_or(3600);

        let mut guard = self.token.write().await;
        *guard = Some(TokenData {
            access_token: access_token.clone(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(expires_in - 60),
        });

        Ok(access_token)
    }
}

#[async_trait]
impl PlatformConnector for RedditConnector {
    fn platform(&self) -> &str { "reddit" }

    async fn validate_credentials(&self) -> bool {
        match self.get_token().await {
            Ok(token) => {
                let resp = self.client
                    .get("https://oauth.reddit.com/api/v1/me")
                    .bearer_auth(&token)
                    .header("User-Agent", "MeemMarketing/0.1")
                    .send().await;
                matches!(resp, Ok(r) if r.status().is_success())
            }
            Err(_) => false,
        }
    }

    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult> {
        let token = self.get_token().await?;
        let url = format!("https://oauth.reddit.com/api/info?id=t3_{}", platform_post_id);
        let resp = self.client
            .get(&url)
            .bearer_auth(&token)
            .header("User-Agent", "MeemMarketing/0.1")
            .send().await?;

        let data: serde_json::Value = resp.json().await?;
        let post = &data["data"]["children"][0]["data"];

        if post.is_null() {
            anyhow::bail!("Reddit post {} not found", platform_post_id);
        }

        Ok(MetricResult {
            views: post["view_count"].as_i64().unwrap_or(0),
            likes: post["score"].as_i64().unwrap_or(0),
            comments: post["num_comments"].as_i64().unwrap_or(0),
            shares: post["num_crossposts"].as_i64().unwrap_or(0),
            custom_metrics: Some(serde_json::json!({
                "upvote_ratio": post["upvote_ratio"],
                "subreddit": post["subreddit"],
                "is_original_content": post["is_original_content"],
                "over_18": post["over_18"],
            })),
            fetched_via: "api".into(),
            ..Default::default()
        })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // Extract from /comments/ABC123/
        let re_pattern = "/comments/([a-zA-Z0-9]+)";
        if let Some(start) = url.find("/comments/") {
            let rest = &url[start + 10..];
            let end = rest.find('/').unwrap_or(rest.len());
            let id = &rest[..end];
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
        let _ = re_pattern; // suppress unused warning
        None
    }

    fn is_api_trackable(&self) -> bool { true }
}
