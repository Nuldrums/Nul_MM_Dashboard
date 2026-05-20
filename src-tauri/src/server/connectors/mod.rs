pub mod reddit;
pub mod hackernews;
pub mod youtube;
pub mod twitter;
pub mod tiktok;
pub mod discord;
pub mod manual;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricResult {
    pub views: i64,
    pub impressions: i64,
    pub likes: i64,
    pub dislikes: i64,
    pub comments: i64,
    pub shares: i64,
    pub saves: i64,
    pub clicks: i64,
    pub watch_time_seconds: Option<i64>,
    pub followers_gained: i64,
    pub custom_metrics: Option<serde_json::Value>,
    pub fetched_via: String,
    pub snapshot_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPost {
    pub platform_post_id: String,
    pub url: String,
    pub title: Option<String>,
    pub posted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub post_type: String,
}

#[async_trait]
pub trait PlatformConnector: Send + Sync {
    fn platform(&self) -> &str;
    async fn validate_credentials(&self) -> bool;
    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult>;
    fn resolve_post_id(&self, url: &str) -> Option<String>;
    fn is_api_trackable(&self) -> bool;

    /// Whether this connector supports auto-feed discovery (listing recent posts by account).
    fn supports_feeds(&self) -> bool { false }

    /// Resolve an account handle (e.g. "@nuldrums") to a stable platform account id.
    /// Returned id is stored on the feed row and reused on subsequent ticks.
    async fn resolve_account_id(&self, _handle: &str) -> anyhow::Result<String> {
        anyhow::bail!("resolve_account_id not implemented for {}", self.platform())
    }

    /// List posts newer than `since_post_id` for the given account + content type.
    /// `content_type` values are connector-specific (e.g. YouTube uses
    /// "long_form" | "short_form" | "live"). When `since_post_id` is None, return
    /// just the single most recent post so the caller can seed the cursor without
    /// inserting historical content.
    async fn list_new_posts(
        &self,
        _account_id: &str,
        _content_type: &str,
        _since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        anyhow::bail!("list_new_posts not implemented for {}", self.platform())
    }

    /// Returns the current follower/subscriber count for the given account, or None
    /// if the platform doesn't expose this. Connectors that need per-user OAuth
    /// (TikTok) look up tokens internally by account_id.
    async fn fetch_follower_count(&self, _account_id: &str) -> anyhow::Result<Option<i64>> {
        Ok(None)
    }
}
