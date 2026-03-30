pub mod reddit;
pub mod hackernews;
pub mod youtube;
pub mod twitter;
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

#[async_trait]
pub trait PlatformConnector: Send + Sync {
    fn platform(&self) -> &str;
    async fn validate_credentials(&self) -> bool;
    async fn fetch_post_metrics(&self, platform_post_id: &str) -> anyhow::Result<MetricResult>;
    fn resolve_post_id(&self, url: &str) -> Option<String>;
    fn is_api_trackable(&self) -> bool;
}
