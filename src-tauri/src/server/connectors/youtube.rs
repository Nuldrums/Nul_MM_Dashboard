use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crate::server::connectors::{PlatformConnector, MetricResult, DiscoveredPost};

const SHORTS_MAX_SECONDS: i64 = 180;

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

    fn supports_feeds(&self) -> bool { true }

    async fn resolve_account_id(&self, handle: &str) -> anyhow::Result<String> {
        // Accept raw channel IDs as-is.
        if handle.starts_with("UC") && handle.len() >= 20 && handle.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Ok(handle.to_string());
        }

        let cleaned = handle.trim_start_matches('@').trim_start_matches('/').trim();
        if cleaned.is_empty() {
            anyhow::bail!("empty handle");
        }

        // Try forHandle first (modern @handles), then forUsername (legacy).
        for param in &["forHandle", "forUsername"] {
            let url = format!(
                "https://www.googleapis.com/youtube/v3/channels?part=id&{}={}&key={}",
                param, cleaned, self.api_key
            );
            let resp = self.client.get(&url).send().await?;
            let data: serde_json::Value = resp.json().await?;
            if let Some(id) = data["items"][0]["id"].as_str() {
                return Ok(id.to_string());
            }
        }
        anyhow::bail!("YouTube channel not found for handle '{}'", handle)
    }

    async fn fetch_follower_count(&self, account_id: &str) -> anyhow::Result<Option<i64>> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/channels?part=statistics&id={}&key={}",
            account_id, self.api_key
        );
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;
        let count = data["items"][0]["statistics"]["subscriberCount"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok());
        Ok(count)
    }

    async fn list_new_posts(
        &self,
        account_id: &str,
        content_type: &str,
        since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        match content_type {
            "long_form" | "short_form" => {
                self.list_uploads(account_id, content_type, since_post_id).await
            }
            "live" => {
                self.list_live(account_id, since_post_id).await
            }
            other => anyhow::bail!("unknown YouTube content_type '{}'", other),
        }
    }
}

impl YouTubeConnector {
    /// Uploads playlist ID is derivable from channel ID: UCxxx → UUxxx.
    /// This has been stable for 10+ years and saves a channels.list quota unit.
    fn uploads_playlist_id(channel_id: &str) -> String {
        if let Some(rest) = channel_id.strip_prefix("UC") {
            format!("UU{}", rest)
        } else {
            channel_id.to_string()
        }
    }

    async fn list_uploads(
        &self,
        channel_id: &str,
        content_type: &str,
        since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        let playlist_id = Self::uploads_playlist_id(channel_id);
        let url = format!(
            "https://www.googleapis.com/youtube/v3/playlistItems?part=contentDetails,snippet&playlistId={}&maxResults=50&key={}",
            playlist_id, self.api_key
        );
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;

        let items = data["items"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            return Ok(vec![]);
        }

        // Walk newest -> oldest, stop at cursor. Order returned is newest first.
        let mut new_video_ids: Vec<String> = Vec::new();
        let mut snippets: std::collections::HashMap<String, (String, Option<DateTime<Utc>>)> = std::collections::HashMap::new();
        for item in &items {
            let vid = item["contentDetails"]["videoId"].as_str().unwrap_or("");
            if vid.is_empty() { continue; }
            if Some(vid) == since_post_id {
                break;
            }
            let title = item["snippet"]["title"].as_str().unwrap_or("").to_string();
            let posted_at = item["contentDetails"]["videoPublishedAt"].as_str()
                .or_else(|| item["snippet"]["publishedAt"].as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            snippets.insert(vid.to_string(), (title, posted_at));
            new_video_ids.push(vid.to_string());
        }

        if new_video_ids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch durations for classification
        let ids_param = new_video_ids.join(",");
        let url = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=contentDetails&id={}&key={}",
            ids_param, self.api_key
        );
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;
        let videos = data["items"].as_array().cloned().unwrap_or_default();

        let want_short = content_type == "short_form";
        let mut out = Vec::new();
        for v in &videos {
            let vid = match v["id"].as_str() { Some(s) => s.to_string(), None => continue };
            let duration_str = v["contentDetails"]["duration"].as_str().unwrap_or("PT0S");
            let secs = parse_iso8601_duration(duration_str);
            let is_short = secs <= SHORTS_MAX_SECONDS;
            if is_short != want_short { continue; }

            let (title, posted_at) = snippets.remove(&vid).unwrap_or_default();
            out.push(DiscoveredPost {
                url: format!("https://www.youtube.com/watch?v={}", vid),
                platform_post_id: vid,
                title: if title.is_empty() { None } else { Some(title) },
                posted_at,
                post_type: if is_short { "video_short".into() } else { "video_long".into() },
            });
        }
        Ok(out)
    }

    async fn list_live(
        &self,
        channel_id: &str,
        since_post_id: Option<&str>,
    ) -> anyhow::Result<Vec<DiscoveredPost>> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/search?part=snippet&channelId={}&type=video&eventType=completed&order=date&maxResults=25&key={}",
            channel_id, self.api_key
        );
        let resp = self.client.get(&url).send().await?;
        let data: serde_json::Value = resp.json().await?;
        let items = data["items"].as_array().cloned().unwrap_or_default();

        let mut out = Vec::new();
        for item in &items {
            let vid = item["id"]["videoId"].as_str().unwrap_or("");
            if vid.is_empty() { continue; }
            if Some(vid) == since_post_id { break; }
            let title = item["snippet"]["title"].as_str().unwrap_or("").to_string();
            let posted_at = item["snippet"]["publishedAt"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            out.push(DiscoveredPost {
                url: format!("https://www.youtube.com/watch?v={}", vid),
                platform_post_id: vid.to_string(),
                title: if title.is_empty() { None } else { Some(title) },
                posted_at,
                post_type: "video_long".into(),
            });
        }
        Ok(out)
    }
}

fn parse_iso8601_duration(s: &str) -> i64 {
    // PT#H#M#S — anything missing is 0
    let body = match s.strip_prefix("PT") { Some(b) => b, None => return 0 };
    let mut total: i64 = 0;
    let mut num = String::new();
    for c in body.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: i64 = num.parse().unwrap_or(0);
            num.clear();
            match c {
                'H' => total += n * 3600,
                'M' => total += n * 60,
                'S' => total += n,
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso8601_durations() {
        assert_eq!(parse_iso8601_duration("PT0S"), 0);
        assert_eq!(parse_iso8601_duration("PT45S"), 45);
        assert_eq!(parse_iso8601_duration("PT3M"), 180);
        assert_eq!(parse_iso8601_duration("PT2M30S"), 150);
        assert_eq!(parse_iso8601_duration("PT1H5M10S"), 3910);
    }

    #[test]
    fn uploads_playlist_id_conversion() {
        assert_eq!(
            YouTubeConnector::uploads_playlist_id("UCabcdefghij"),
            "UUabcdefghij"
        );
    }
}
