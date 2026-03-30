use async_trait::async_trait;
use crate::server::connectors::{PlatformConnector, MetricResult};

pub struct DiscordConnector;

impl DiscordConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl PlatformConnector for DiscordConnector {
    fn platform(&self) -> &str { "discord" }
    async fn validate_credentials(&self) -> bool { true }

    async fn fetch_post_metrics(&self, _platform_post_id: &str) -> anyhow::Result<MetricResult> {
        Ok(MetricResult { fetched_via: "manual".into(), ..Default::default() })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        // Extract message_id from discord.com/channels/server/channel/message
        url.rsplit('/').next().map(|s| s.to_string())
    }

    fn is_api_trackable(&self) -> bool { false }
}
