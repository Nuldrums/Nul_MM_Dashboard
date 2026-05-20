use std::sync::Arc;
use std::collections::HashMap;
use crate::server::AppState;
use crate::server::connectors::*;
use crate::server::connectors::{hackernews::HackerNewsConnector, reddit::RedditConnector,
    youtube::YouTubeConnector, twitter::TwitterConnector, tiktok::TikTokConnector,
    discord::DiscordConnector, manual::ManualConnector};

pub async fn collect_all(state: &Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting metric collection for all tracked posts");

    let connectors = build_connectors(state).await;

    // Discover new posts from auto-feeds before listing posts to fetch, so any
    // newly-discovered posts are picked up by the same tick.
    if let Err(e) = discover_feed_posts(state, &connectors).await {
        tracing::warn!("Feed discovery failed: {}", e);
    }

    // Get all API-tracked posts
    let posts: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, platform, platform_post_id, url FROM posts WHERE is_api_tracked = 1"
    ).fetch_all(&state.db).await?;

    let mut fetched = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for (post_id, platform, platform_post_id, url) in &posts {
        let connector = match connectors.get(platform.as_str()) {
            Some(c) => c,
            None => {
                tracing::debug!("No connector for platform '{}', skipping post {}", platform, post_id);
                skipped += 1;
                continue;
            }
        };

        if !connector.is_api_trackable() {
            skipped += 1;
            continue;
        }

        // Use stored platform_post_id, or derive from URL as fallback
        let resolved_id: Option<String> = match platform_post_id {
            Some(pid) if !pid.is_empty() => Some(pid.clone()),
            _ => url.as_deref().and_then(|u| connector.resolve_post_id(u)),
        };

        let pid = match resolved_id {
            Some(p) => p,
            None => {
                tracing::warn!("Post {} ({}): no platform_post_id and could not derive from URL {:?}",
                    post_id, platform, url);
                skipped += 1;
                continue;
            }
        };

        // Backfill the resolved id so future runs skip this work
        if platform_post_id.as_deref().unwrap_or("").is_empty() {
            let _ = sqlx::query("UPDATE posts SET platform_post_id = ? WHERE id = ?")
                .bind(&pid).bind(post_id).execute(&state.db).await;
        }

        match connector.fetch_post_metrics(&pid).await {
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
    tracing::info!("Metric collection summary: fetched={}, failed={}, skipped={}", fetched, failed, skipped);

    // Update system state
    sqlx::query(
        "INSERT INTO system_state (key, value, updated_at) VALUES ('last_metric_fetch', datetime('now'), datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = datetime('now'), updated_at = datetime('now')"
    ).execute(&state.db).await?;

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

async fn discover_feed_posts(
    state: &Arc<AppState>,
    connectors: &HashMap<&'static str, Box<dyn PlatformConnector>>,
) -> anyhow::Result<()> {
    // Join campaign_feeds with profile_accounts so we get platform/handle/account_id in one shot.
    let feeds: Vec<(String, String, String, String, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT cf.id, cf.campaign_id, cf.profile_account_id,
                pa.platform, pa.account_handle, COALESCE(pa.account_id, '') AS account_id_str,
                pa.account_id,
                cf.content_type, cf.last_seen_post_id
         FROM campaign_feeds cf
         JOIN profile_accounts pa ON pa.id = cf.profile_account_id
         WHERE cf.is_active = 1 AND pa.is_active = 1"
    ).fetch_all(&state.db).await?;

    if feeds.is_empty() {
        return Ok(());
    }
    tracing::info!("Checking {} active feeds for new posts", feeds.len());

    let mut total_discovered = 0;

    for (feed_id, campaign_id, profile_account_id, platform, account_handle, _account_id_str, account_id_opt, content_type, last_seen) in feeds {
        let connector = match connectors.get(platform.as_str()) {
            Some(c) if c.supports_feeds() => c,
            _ => {
                tracing::debug!("Feed {}: connector for '{}' missing or unsupported, skipping", feed_id, platform);
                continue;
            }
        };

        // Resolve account_id if we don't have it yet (cached on profile_accounts).
        let account_id = match account_id_opt {
            Some(id) if !id.is_empty() => id,
            _ => match connector.resolve_account_id(&account_handle).await {
                Ok(id) => {
                    let _ = sqlx::query("UPDATE profile_accounts SET account_id = ? WHERE id = ?")
                        .bind(&id).bind(&profile_account_id).execute(&state.db).await;
                    id
                }
                Err(e) => {
                    tracing::warn!("Feed {}: failed to resolve account '{}': {}", feed_id, account_handle, e);
                    let _ = sqlx::query("UPDATE campaign_feeds SET last_error = ?, last_checked_at = datetime('now') WHERE id = ?")
                        .bind(format!("Could not resolve account: {}", e)).bind(&feed_id).execute(&state.db).await;
                    continue;
                }
            }
        };

        match connector.list_new_posts(&account_id, &content_type, last_seen.as_deref()).await {
            Ok(discovered) => {
                let mut newest_in_batch: Option<(String, Option<String>)> = None;
                for post in &discovered {
                    let posted_at_str = post.posted_at.map(|dt| dt.to_rfc3339());

                    // Dedup: if this post already exists in the same campaign (e.g. a previous
                    // feed run discovered it before being recreated), don't double-insert.
                    // Backfill profile_account_id on the existing row if it's null (orphaned
                    // by a prior account deletion).
                    let existing: Option<(String, Option<String>)> = sqlx::query_as(
                        "SELECT id, profile_account_id FROM posts
                         WHERE campaign_id = ? AND platform = ? AND platform_post_id = ?
                         LIMIT 1"
                    )
                        .bind(&campaign_id).bind(&platform).bind(&post.platform_post_id)
                        .fetch_optional(&state.db).await.ok().flatten();

                    if let Some((existing_id, existing_pa)) = existing {
                        if existing_pa.is_none() {
                            let _ = sqlx::query(
                                "UPDATE posts SET profile_account_id = ? WHERE id = ?"
                            ).bind(&profile_account_id).bind(&existing_id).execute(&state.db).await;
                            tracing::info!("Feed {}: re-linked orphaned post {} to account {}", feed_id, post.platform_post_id, profile_account_id);
                        }
                        if newest_in_batch.is_none() {
                            newest_in_batch = Some((post.platform_post_id.clone(), posted_at_str.clone()));
                        }
                        continue;
                    }

                    let new_id = uuid::Uuid::new_v4().to_string();
                    let insert_result = sqlx::query(
                        "INSERT INTO posts (id, campaign_id, platform, post_type, platform_post_id,
                         url, title, posted_at, is_api_tracked, profile_account_id)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?)"
                    )
                        .bind(&new_id).bind(&campaign_id).bind(&platform).bind(&post.post_type)
                        .bind(&post.platform_post_id).bind(&post.url).bind(&post.title)
                        .bind(&posted_at_str).bind(&profile_account_id)
                        .execute(&state.db).await;

                    if let Err(e) = insert_result {
                        tracing::warn!("Feed {}: failed to insert discovered post {}: {}", feed_id, post.platform_post_id, e);
                        continue;
                    }
                    total_discovered += 1;
                    tracing::info!("Feed {}: discovered new post {} ({})", feed_id, post.platform_post_id, post.post_type);

                    // First entry in `discovered` is the newest (list_new_posts returns newest-first)
                    if newest_in_batch.is_none() {
                        newest_in_batch = Some((post.platform_post_id.clone(), posted_at_str));
                    }
                }

                if let Some((newest_id, newest_posted)) = newest_in_batch {
                    let _ = sqlx::query(
                        "UPDATE campaign_feeds SET last_seen_post_id = ?, last_seen_posted_at = ?,
                         last_checked_at = datetime('now'), last_error = NULL WHERE id = ?"
                    )
                        .bind(&newest_id).bind(&newest_posted).bind(&feed_id)
                        .execute(&state.db).await;
                } else {
                    let _ = sqlx::query(
                        "UPDATE campaign_feeds SET last_checked_at = datetime('now'), last_error = NULL WHERE id = ?"
                    ).bind(&feed_id).execute(&state.db).await;
                }
            }
            Err(e) => {
                tracing::warn!("Feed {}: list_new_posts failed: {}", feed_id, e);
                let _ = sqlx::query(
                    "UPDATE campaign_feeds SET last_error = ?, last_checked_at = datetime('now') WHERE id = ?"
                ).bind(e.to_string()).bind(&feed_id).execute(&state.db).await;
            }
        }
    }

    tracing::info!("Feed discovery complete: {} new posts inserted", total_discovered);

    // Refresh follower counts for accounts whose follower_count_at is stale (>1 hour)
    // or null. Throttled to avoid burning quota on every metric tick.
    if let Err(e) = refresh_follower_counts(state, connectors).await {
        tracing::warn!("Follower-count refresh failed: {}", e);
    }

    Ok(())
}

const FOLLOWER_REFRESH_INTERVAL_HOURS: i64 = 1;

async fn refresh_follower_counts(
    state: &Arc<AppState>,
    connectors: &HashMap<&'static str, Box<dyn PlatformConnector>>,
) -> anyhow::Result<()> {
    // Only refresh accounts that actually have at least one active feed somewhere.
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(FOLLOWER_REFRESH_INTERVAL_HOURS);
    let cutoff_str = cutoff.to_rfc3339();

    let accounts: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT pa.id, pa.platform, pa.account_id
         FROM profile_accounts pa
         JOIN campaign_feeds cf ON cf.profile_account_id = pa.id
         WHERE pa.is_active = 1 AND cf.is_active = 1
           AND (pa.follower_count_at IS NULL OR pa.follower_count_at < ?)"
    ).bind(&cutoff_str).fetch_all(&state.db).await?;

    for (account_uuid, platform, account_id_opt) in accounts {
        let connector = match connectors.get(platform.as_str()) {
            Some(c) => c,
            None => continue,
        };
        let Some(account_id) = account_id_opt else { continue };

        match connector.fetch_follower_count(&account_id).await {
            Ok(Some(count)) => {
                let _ = sqlx::query(
                    "UPDATE profile_accounts SET follower_count = ?, follower_count_at = datetime('now') WHERE id = ?"
                ).bind(count).bind(&account_uuid).execute(&state.db).await;
                tracing::info!("Refreshed follower count for {} account {}: {}", platform, account_uuid, count);
            }
            Ok(None) => {
                tracing::debug!("Connector for {} returned no follower count", platform);
            }
            Err(e) => {
                tracing::warn!("Follower-count fetch failed for {} account {}: {}", platform, account_uuid, e);
            }
        }
    }
    Ok(())
}

pub async fn build_connectors(state: &Arc<AppState>) -> HashMap<&'static str, Box<dyn PlatformConnector>> {
    let mut map: HashMap<&'static str, Box<dyn PlatformConnector>> = HashMap::new();

    let db_creds = load_db_credentials(&state.db).await;
    let cred = |platform: &str, field: &str| -> Option<String> {
        db_creds
            .get(platform)
            .and_then(|c| c.get(field))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    // HackerNews is always available
    map.insert("hackernews", Box::new(HackerNewsConnector::new(state.http_client.clone())));

    // Reddit
    let reddit_id = cred("reddit", "client_id").unwrap_or_else(|| state.settings.reddit_client_id.clone());
    if !reddit_id.is_empty() {
        map.insert("reddit", Box::new(RedditConnector::new(
            state.http_client.clone(),
            reddit_id,
            cred("reddit", "client_secret").unwrap_or_else(|| state.settings.reddit_client_secret.clone()),
            cred("reddit", "username").unwrap_or_else(|| state.settings.reddit_username.clone()),
            cred("reddit", "password").unwrap_or_else(|| state.settings.reddit_password.clone()),
        )));
    }

    // YouTube
    let yt_key = cred("youtube", "api_key").unwrap_or_else(|| state.settings.youtube_api_key.clone());
    if !yt_key.is_empty() {
        map.insert("youtube", Box::new(YouTubeConnector::new(
            state.http_client.clone(),
            yt_key,
        )));
    }

    // X (Twitter). Settings stores credentials under the legacy "twitter" key, but posts and
    // the UI use "x" everywhere — register the connector under "x" so the post lookup matches.
    let tw_token = cred("twitter", "bearer_token").unwrap_or_else(|| state.settings.twitter_bearer_token.clone());
    if !tw_token.is_empty() {
        map.insert("x", Box::new(TwitterConnector::new(
            state.http_client.clone(),
            tw_token,
        )));
    }

    // TikTok — needs app credentials AND per-account OAuth tokens (latter stored in profile_accounts).
    let tt_key = cred("tiktok", "client_key").unwrap_or_default();
    let tt_secret = cred("tiktok", "client_secret").unwrap_or_default();
    map.insert("tiktok", Box::new(TikTokConnector::new(
        state.http_client.clone(),
        tt_key,
        tt_secret,
        state.db.clone(),
    )));

    // Manual stubs for platforms without API access
    map.insert("discord", Box::new(DiscordConnector::new()));
    map.insert("producthunt", Box::new(ManualConnector::new("producthunt")));
    map.insert("instagram", Box::new(ManualConnector::new("instagram")));
    map.insert("linkedin", Box::new(ManualConnector::new("linkedin")));

    map
}

async fn load_db_credentials(pool: &sqlx::SqlitePool) -> HashMap<String, serde_json::Value> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT platform, credentials FROM platform_configs WHERE is_enabled = 1"
    ).fetch_all(pool).await.unwrap_or_default();

    rows.into_iter()
        .filter_map(|(platform, creds)| {
            let creds_str = creds?;
            let value: serde_json::Value = serde_json::from_str(&creds_str).ok()?;
            Some((platform, value))
        })
        .collect()
}
