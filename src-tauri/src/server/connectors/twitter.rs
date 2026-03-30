use async_trait::async_trait;
use crate::server::connectors::{PlatformConnector, MetricResult};

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
    fn platform(&self) -> &str { "twitter" }

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
}
