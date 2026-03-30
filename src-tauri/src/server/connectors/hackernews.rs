use async_trait::async_trait;
use crate::server::connectors::{PlatformConnector, MetricResult};

const HN_API_BASE: &str = "https://hacker-news.firebaseio.com/v0";

pub struct HackerNewsConnector {
    client: reqwest::Client,
}

impl HackerNewsConnector {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlatformConnector for HackerNewsConnector {
    fn platform(&self) -> &str { "hackernews" }

    async fn validate_credentials(&self) -> bool {
        match self.client.get(format!("{}/topstories.json", HN_API_BASE))
            .timeout(std::time::Duration::from_secs(10))
            .send().await
        {
            Ok(r) => r.status().is_success(),
            Err(e) => {
                tracing::warn!("HN API validation failed: {}", e);
                false
            }
        }
    }

    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult> {
        let url = format!("{}/item/{}.json", HN_API_BASE, platform_post_id);
        let resp = self.client.get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send().await?;
        let data: serde_json::Value = resp.json().await?;

        if data.is_null() {
            anyhow::bail!("HN item {} not found", platform_post_id);
        }

        Ok(MetricResult {
            likes: data["score"].as_i64().unwrap_or(0),
            comments: data["descendants"].as_i64().unwrap_or(0),
            custom_metrics: Some(serde_json::json!({
                "hn_type": data["type"],
                "hn_by": data["by"],
                "hn_time": data["time"],
            })),
            fetched_via: "api".into(),
            ..Default::default()
        })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // Extract from ?id=12345
        if let Some(pos) = url.find("id=") {
            let rest = &url[pos + 3..];
            let end = rest.find('&').unwrap_or(rest.len());
            let id = &rest[..end];
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
        None
    }

    fn is_api_trackable(&self) -> bool { true }
}
