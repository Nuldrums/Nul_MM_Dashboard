use std::sync::Arc;
use std::collections::HashMap;
use crate::server::AppState;
use crate::server::connectors::*;
use crate::server::connectors::{hackernews::HackerNewsConnector, reddit::RedditConnector,
    youtube::YouTubeConnector, twitter::TwitterConnector, discord::DiscordConnector,
    manual::ManualConnector};

pub async fn collect_all(state: &Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting metric collection for all tracked posts");

    let connectors = build_connectors(state);

    // Get all API-tracked posts
    let posts: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, platform, platform_post_id FROM posts WHERE is_api_tracked = 1"
    ).fetch_all(&state.db).await?;

    let mut fetched = 0;
    let mut failed = 0;

    for (post_id, platform, platform_post_id) in &posts {
        let platform_post_id = match platform_post_id {
            Some(pid) if !pid.is_empty() => pid,
            _ => {
                tracing::debug!("Post {} has no platform_post_id, skipping", post_id);
                continue;
            }
        };

        let connector = match connectors.get(platform.as_str()) {
            Some(c) => c,
            None => {
                // Fall back to manual connector
                tracing::debug!("No connector for platform '{}', skipping post {}", platform, post_id);
                continue;
            }
        };

        if !connector.is_api_trackable() {
            continue;
        }

        match connector.fetch_post_metrics(platform_post_id).await {
            Ok(metrics) => {
                upsert_snapshot(&state.db, post_id, &metrics).await?;
                fetched += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch metrics for post {} ({}): {}", post_id, platform, e);
                failed += 1;
            }
        }
    }

    // Update system state
    sqlx::query(
        "INSERT INTO system_state (key, value, updated_at) VALUES ('last_metric_fetch', datetime('now'), datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = datetime('now'), updated_at = datetime('now')"
    ).execute(&state.db).await?;

    tracing::info!("Metric collection complete: fetched={}, failed={}", fetched, failed);
    Ok(())
}

async fn upsert_snapshot(
    pool: &sqlx::SqlitePool,
    post_id: &str,
    metrics: &MetricResult,
) -> anyhow::Result<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let custom_json = metrics.custom_metrics.as_ref().map(|v| v.to_string());

    sqlx::query(
        "INSERT INTO metric_snapshots (post_id, snapshot_date, views, impressions, likes, dislikes,
         comments, shares, saves, clicks, watch_time_seconds, followers_gained, custom_metrics, fetched_via)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(post_id, snapshot_date) DO UPDATE SET
           views=excluded.views, impressions=excluded.impressions, likes=excluded.likes,
           dislikes=excluded.dislikes, comments=excluded.comments, shares=excluded.shares,
           saves=excluded.saves, clicks=excluded.clicks, watch_time_seconds=excluded.watch_time_seconds,
           followers_gained=excluded.followers_gained, custom_metrics=excluded.custom_metrics,
           fetched_via=excluded.fetched_via"
    )
        .bind(post_id).bind(&today)
        .bind(metrics.views).bind(metrics.impressions)
        .bind(metrics.likes).bind(metrics.dislikes)
        .bind(metrics.comments).bind(metrics.shares)
        .bind(metrics.saves).bind(metrics.clicks)
        .bind(metrics.watch_time_seconds).bind(metrics.followers_gained)
        .bind(&custom_json).bind(&metrics.fetched_via)
        .execute(pool).await?;

    Ok(())
}

fn build_connectors(state: &Arc<AppState>) -> HashMap<&'static str, Box<dyn PlatformConnector>> {
    let mut map: HashMap<&'static str, Box<dyn PlatformConnector>> = HashMap::new();

    // HackerNews is always available
    map.insert("hackernews", Box::new(HackerNewsConnector::new(state.http_client.clone())));

    // Reddit
    if !state.settings.reddit_client_id.is_empty() {
        map.insert("reddit", Box::new(RedditConnector::new(
            state.http_client.clone(),
            state.settings.reddit_client_id.clone(),
            state.settings.reddit_client_secret.clone(),
            state.settings.reddit_username.clone(),
            state.settings.reddit_password.clone(),
        )));
    }

    // YouTube
    if !state.settings.youtube_api_key.is_empty() {
        map.insert("youtube", Box::new(YouTubeConnector::new(
            state.http_client.clone(),
            state.settings.youtube_api_key.clone(),
        )));
    }

    // Twitter
    if !state.settings.twitter_bearer_token.is_empty() {
        map.insert("twitter", Box::new(TwitterConnector::new(
            state.http_client.clone(),
            state.settings.twitter_bearer_token.clone(),
        )));
    }

    // Manual stubs for platforms without API access
    map.insert("discord", Box::new(DiscordConnector::new()));
    map.insert("producthunt", Box::new(ManualConnector::new("producthunt")));
    map.insert("tiktok", Box::new(ManualConnector::new("tiktok")));
    map.insert("instagram", Box::new(ManualConnector::new("instagram")));
    map.insert("linkedin", Box::new(ManualConnector::new("linkedin")));

    map
}
