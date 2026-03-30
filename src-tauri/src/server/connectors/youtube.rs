use async_trait::async_trait;
use crate::server::connectors::{PlatformConnector, MetricResult};

pub struct YouTubeConnector {
    client: reqwest::Client,
    api_key: String,
}

impl YouTubeConnector {
    pub fn new(client: reqwest::Client, api_key: String) -> Self {
        Self { client, api_key }
    }
}

#[async_trait]
impl PlatformConnector for YouTubeConnector {
    fn platform(&self) -> &str { "youtube" }

    async fn validate_credentials(&self) -> bool {
        // Test with a known video
        let url = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=statistics&id=dQw4w9WgXcQ&key={}",
            self.api_key
        );
        match self.client.get(&url).send().await {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        }
    }

    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=statistics&id={}&key={}",
            platform_post_id, self.api_key
        );
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;

        let stats = &data["items"][0]["statistics"];
        if stats.is_null() {
            anyhow::bail!("YouTube video {} not found", platform_post_id);
        }

        let parse_i64 = |key: &str| -> i64 {
            stats[key].as_str().and_then(|s| s.parse().ok()).unwrap_or(0)
        };

        Ok(MetricResult {
            views: parse_i64("viewCount"),
            likes: parse_i64("likeCount"),
            dislikes: parse_i64("dislikeCount"),
            comments: parse_i64("commentCount"),
            saves: parse_i64("favoriteCount"),
            fetched_via: "api".into(),
            ..Default::default()
        })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // watch?v=ID, /shorts/ID, youtu.be/ID
        if let Some(pos) = url.find("v=") {
            let rest = &url[pos + 2..];
            let end = rest.find('&').unwrap_or(rest.len());
            return Some(rest[..end].to_string());
        }
        if let Some(pos) = url.find("/shorts/") {
            let rest = &url[pos + 8..];
            let end = rest.find('?').or_else(|| rest.find('/')).unwrap_or(rest.len());
            return Some(rest[..end].to_string());
        }
        if url.contains("youtu.be/") {
            if let Some(pos) = url.find("youtu.be/") {
                let rest = &url[pos + 9..];
                let end = rest.find('?').unwrap_or(rest.len());
                return Some(rest[..end].to_string());
            }
        }
        None
    }

    fn is_api_trackable(&self) -> bool { true }
}
