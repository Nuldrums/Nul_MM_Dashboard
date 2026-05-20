use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crate::server::connectors::{PlatformConnector, MetricResult, DiscoveredPost};

pub struct TwitterConnector {
    client: reqwest::Client,
    bearer_token: String,
}

impl TwitterConnector {
    pub fn new(client: reqwest::Client, bearer_token: String) -> Self {
        Self { client, bearer_token }
    }
}

#[async_trait]
impl PlatformConnector for TwitterConnector {
    fn platform(&self) -> &str { "x" }

    async fn validate_credentials(&self) -> bool {
        let resp = self.client
            .get("https://api.twitter.com/2/users/me")
            .bearer_auth(&self.bearer_token)
            .send().await;
        match resp {
            Ok(r) => r.status().as_u16() != 401,
            Err(_) => false,
        }
    }

    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult> {
        let url = format!(
            "https://api.twitter.com/2/tweets/{}?tweet.fields=public_metrics",
            platform_post_id
        );
        let resp = self.client
            .get(&url)
            .bearer_auth(&self.bearer_token)
            .send().await?;

        let data: serde_json::Value = resp.json().await?;
        let metrics = &data["data"]["public_metrics"];

        if metrics.is_null() {
            anyhow::bail!("Twitter post {} not found or no metrics", platform_post_id);
        }

        let retweets = metrics["retweet_count"].as_i64().unwrap_or(0);
        let quotes = metrics["quote_count"].as_i64().unwrap_or(0);

        Ok(MetricResult {
            views: metrics["impression_count"].as_i64().unwrap_or(0),
            impressions: metrics["impression_count"].as_i64().unwrap_or(0),
            likes: metrics["like_count"].as_i64().unwrap_or(0),
            comments: metrics["reply_count"].as_i64().unwrap_or(0),
            shares: retweets + quotes,
            saves: metrics["bookmark_count"].as_i64().unwrap_or(0),
            custom_metrics: Some(serde_json::json!({
                "retweet_count": retweets,
                "quote_count": quotes,
                "bookmark_count": metrics["bookmark_count"],
            })),
            fetched_via: "api".into(),
            ..Default::default()
        })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // Extract from /status/1234567890
        if let Some(pos) = url.find("/status/") {
            let rest = &url[pos + 8..];
            let end = rest.find('?').or_else(|| rest.find('/')).unwrap_or(rest.len());
            let id = &rest[..end];
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
        None
    }

    fn is_api_trackable(&self) -> bool { true }

    fn supports_feeds(&self) -> bool { true }

    async fn resolve_account_id(&self, handle: &str) -> anyhow::Result<String> {
        let cleaned = handle.trim_start_matches('@').trim();
        if cleaned.is_empty() {
            anyhow::bail!("empty handle");
        }
        // Numeric IDs pass straight through.
        if cleaned.chars().all(|c| c.is_ascii_digit()) && cleaned.len() >= 5 {
            return Ok(cleaned.to_string());
        }

        let url = format!("https://api.twitter.com/2/users/by/username/{}", cleaned);
        let resp = self.client.get(&url).bearer_auth(&self.bearer_token).send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;

        if !status.is_success() {
            let title = data["title"].as_str().unwrap_or("");
            let detail = data["detail"].as_str().unwrap_or("");
            anyhow::bail!("X API error resolving '{}': {} {} {}", cleaned, status, title, detail);
        }
        let id = data["data"]["id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("X user '{}' not found", cleaned))?;
        Ok(id.to_string())
    }

    async fn fetch_follower_count(&self, account_id: &str) -> anyhow::Result<Option<i64>> {
        let url = format!("https://api.twitter.com/2/users/{}?user.fields=public_metrics", account_id);
        let resp = self.client.get(&url).bearer_auth(&self.bearer_token).send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            anyhow::bail!("X API error fetching user {}: {} {}", account_id, status, data);
        }
        Ok(data["data"]["public_metrics"]["followers_count"].as_i64())
    }

    async fn list_new_posts(
        &self,
        account_id: &str,
        content_type: &str,
        since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        // Build query params per content_type. X excludes are server-side; "media" needs client filter.
        let mut params = vec![
            ("max_results", "10".to_string()),
            ("tweet.fields", "created_at,attachments,entities,in_reply_to_user_id,referenced_tweets".to_string()),
        ];
        match content_type {
            "tweets" => {
                params.push(("exclude", "retweets,replies".to_string()));
            }
            "replies" => {
                params.push(("exclude", "retweets".to_string()));
            }
            "media" => {
                params.push(("exclude", "retweets,replies".to_string()));
                params.push(("expansions", "attachments.media_keys".to_string()));
            }
            "broadcasts" => {
                params.push(("exclude", "retweets,replies".to_string()));
            }
            other => anyhow::bail!("unknown X content_type '{}'", other),
        }
        if let Some(sid) = since_post_id {
            // since_id only works with valid numeric tweet IDs.
            if sid.chars().all(|c| c.is_ascii_digit()) {
                params.push(("since_id", sid.to_string()));
            }
        }

        let url = format!("https://api.twitter.com/2/users/{}/tweets", account_id);
        let resp = self.client.get(&url)
            .bearer_auth(&self.bearer_token)
            .query(&params)
            .send().await?;
        let status = resp.status();
        let data: serde_json::Value = resp.json().await?;

        if !status.is_success() {
            let title = data["title"].as_str().unwrap_or("");
            let detail = data["detail"].as_str().unwrap_or("");
            anyhow::bail!("X API error listing tweets ({}): {} {}", status, title, detail);
        }

        let tweets = data["data"].as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        for tw in &tweets {
            let id = tw["id"].as_str().unwrap_or("");
            if id.is_empty() { continue; }

            // Apply client-side filters that the API can't express directly.
            match content_type {
                "replies" => {
                    if tw["in_reply_to_user_id"].is_null() { continue; }
                }
                "media" => {
                    if tw["attachments"]["media_keys"].as_array().is_none() { continue; }
                }
                "broadcasts" => {
                    let is_broadcast = tw["entities"]["urls"].as_array()
                        .map(|urls| urls.iter().any(|u| {
                            let target = u["expanded_url"].as_str()
                                .or_else(|| u["unwound_url"].as_str())
                                .unwrap_or("");
                            target.contains("broadcasts.twitter.com")
                                || target.contains("/i/broadcasts/")
                                || target.contains("pscp.tv")
                        }))
                        .unwrap_or(false);
                    if !is_broadcast { continue; }
                }
                _ => {}
            }

            // since_id is exclusive on X — but double-check just in case.
            if Some(id) == since_post_id { continue; }

            let text = tw["text"].as_str().unwrap_or("").to_string();
            let title = if text.len() > 120 {
                Some(format!("{}…", &text[..120]))
            } else if text.is_empty() {
                None
            } else {
                Some(text)
            };
            let posted_at = tw["created_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let post_type = match content_type {
                "media" => "image",
                "broadcasts" => "video_long",
                _ => "text",
            };
            out.push(DiscoveredPost {
                url: format!("https://x.com/i/web/status/{}", id),
                platform_post_id: id.to_string(),
                title,
                posted_at,
                post_type: post_type.into(),
            });
        }
        Ok(out)
    }
}
