use async_trait::async_trait;
use crate::server::connectors::{PlatformConnector, MetricResult};

pub struct ManualConnector {
    platform_name: String,
}

impl ManualConnector {
    pub fn new(platform: &str) -> Self {
        Self { platform_name: platform.to_string() }
    }
}

#[async_trait]
impl PlatformConnector for ManualConnector {
    fn platform(&self) -> &str { &self.platform_name }
    async fn validate_credentials(&self) -> bool { true }

    async fn fetch_post_metrics(&self, _platform_post_id: &str) -> anyhow::Result<MetricResult> {
        Ok(MetricResult { fetched_via: "manual".into(), ..Default::default() })
    }

    fn resolve_post_id(&self, url: &str) -> Option<String> {
        Some(url.to_string())
    }

    fn is_api_trackable(&self) -> bool { false }
}
