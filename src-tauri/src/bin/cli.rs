/// MEEM Marketing CLI
///
/// A complete command-line interface for all backend API operations.
/// Designed to be AI-pilotable — every endpoint is accessible via structured
/// subcommands with JSON output for easy parsing.
///
/// Usage:
///   meem-cli [--base-url URL] <command> <subcommand> [args...]
///
/// Examples:
///   meem-cli health
///   meem-cli profile list
///   meem-cli profile create --name "My Brand"
///   meem-cli product create --name "Widget" --type "SaaS" --profile-id <id>
///   meem-cli campaign create --name "Launch" --product-id <id>
///   meem-cli post create --campaign-id <id> --platform reddit --post-type link --url "https://..."
///   meem-cli metrics fetch
///   meem-cli analytics overview
///   meem-cli ai trigger
///   meem-cli ai status
///   meem-cli settings get

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "meem-cli", about = "MEEM Marketing CLI — AI-pilotable interface")]
struct Cli {
    /// Base URL of the backend API
    #[arg(long, default_value = "http://127.0.0.1:31415", global = true)]
    base_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check server health
    Health,

    /// Startup check (detailed system status)
    StartupCheck,

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Product management
    Product {
        #[command(subcommand)]
        action: ProductAction,
    },

    /// Campaign management
    Campaign {
        #[command(subcommand)]
        action: CampaignAction,
    },

    /// Post management
    Post {
        #[command(subcommand)]
        action: PostAction,
    },

    /// Metrics operations
    Metrics {
        #[command(subcommand)]
        action: MetricsAction,
    },

    /// Analytics queries
    Analytics {
        #[command(subcommand)]
        action: AnalyticsAction,
    },

    /// AI analysis operations
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },

    /// Settings management
    Settings {
        #[command(subcommand)]
        action: SettingsAction,
    },

    /// Run the backend server standalone (no Tauri)
    Serve,
}

// ── Profile ──────────────────────────────────────────────

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,
    /// Create a new profile
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        avatar_color: Option<String>,
    },
    /// Update a profile
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        avatar_color: Option<String>,
    },
    /// Delete a profile
    Delete {
        #[arg(long)]
        id: String,
    },
}

// ── Product ──────────────────────────────────────────────

#[derive(Subcommand)]
enum ProductAction {
    /// List products (optionally filter by profile)
    List {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Create a product
    Create {
        #[arg(long)]
        name: String,
        /// Product type (e.g. "SaaS", "Physical", "Service")
        #[arg(long, rename_all = "kebab-case")]
        r#type: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        price: Option<f64>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Update a product
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        price: Option<f64>,
        #[arg(long)]
        tags: Option<String>,
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Delete a product
    Delete {
        #[arg(long)]
        id: String,
    },
}

// ── Campaign ──────────────────────────────────────────────

#[derive(Subcommand)]
enum CampaignAction {
    /// List campaigns (optionally filter by profile)
    List {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Get a single campaign with its posts
    Get {
        #[arg(long)]
        id: String,
    },
    /// Create a campaign
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        product_id: String,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long)]
        target_audience: Option<String>,
        #[arg(long)]
        start_date: Option<String>,
        #[arg(long)]
        end_date: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Update a campaign
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        product_id: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long)]
        target_audience: Option<String>,
        #[arg(long)]
        start_date: Option<String>,
        #[arg(long)]
        end_date: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Archive (soft-delete) a campaign
    Delete {
        #[arg(long)]
        id: String,
    },
}

// ── Post ──────────────────────────────────────────────

#[derive(Subcommand)]
enum PostAction {
    /// List posts for a campaign
    List {
        #[arg(long)]
        campaign_id: String,
    },
    /// Create a post
    Create {
        #[arg(long)]
        campaign_id: String,
        /// Platform: reddit, hackernews, youtube, twitter, discord, manual
        #[arg(long)]
        platform: String,
        /// Post type: link, text, video, image, comment, thread
        #[arg(long)]
        post_type: String,
        #[arg(long)]
        platform_post_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body_preview: Option<String>,
        #[arg(long)]
        target_community: Option<String>,
        #[arg(long)]
        posted_at: Option<String>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
        /// Whether this post should be tracked via platform API
        #[arg(long, default_value_t = false)]
        is_api_tracked: bool,
    },
    /// Update a post
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long)]
        post_type: Option<String>,
        #[arg(long)]
        platform_post_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body_preview: Option<String>,
        #[arg(long)]
        target_community: Option<String>,
        #[arg(long)]
        posted_at: Option<String>,
        #[arg(long)]
        tags: Option<String>,
        #[arg(long)]
        is_api_tracked: Option<bool>,
    },
    /// Delete a post
    Delete {
        #[arg(long)]
        id: String,
    },
}

// ── Metrics ──────────────────────────────────────────────

#[derive(Subcommand)]
enum MetricsAction {
    /// Get metrics for a specific post
    Post {
        #[arg(long)]
        post_id: String,
    },
    /// Get aggregated metrics for a campaign
    Campaign {
        #[arg(long)]
        campaign_id: String,
    },
    /// Trigger metric fetching from all platforms
    Fetch,
    /// Check metric fetch status
    Status,
}

// ── Analytics ──────────────────────────────────────────────

#[derive(Subcommand)]
enum AnalyticsAction {
    /// Overview statistics
    Overview {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Per-platform breakdown
    Platforms {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Per-post-type breakdown
    PostTypes {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Time-series trends
    Trends {
        #[arg(long)]
        profile_id: Option<String>,
    },
}

// ── AI ──────────────────────────────────────────────

#[derive(Subcommand)]
enum AiAction {
    /// Get latest analyses for all active campaigns
    Latest {
        #[arg(long)]
        profile_id: Option<String>,
    },
    /// Get analyses for a specific campaign
    Campaign {
        #[arg(long)]
        campaign_id: String,
    },
    /// Trigger the full analysis pipeline
    Trigger,
    /// Check analysis status
    Status,
    /// Get cross-campaign recommendations
    Recommendations,
    /// Get cross-campaign insight summary
    Insight,
    /// Query the knowledge base
    KbQuery {
        #[arg(long)]
        q: String,
    },
    /// Get knowledge base statistics
    KbStats,
}

// ── Settings ──────────────────────────────────────────────

#[derive(Subcommand)]
enum SettingsAction {
    /// Get all settings
    Get,
    /// Update platform configuration
    UpdatePlatform {
        /// Platform name (e.g. "reddit", "youtube")
        #[arg(long)]
        platform: String,
        /// JSON string of credentials
        #[arg(long)]
        credentials: Option<String>,
        #[arg(long)]
        is_enabled: Option<bool>,
        /// JSON string of config
        #[arg(long)]
        config: Option<String>,
    },
    /// Update general settings
    UpdateGeneral {
        #[arg(long)]
        data_dir: Option<String>,
        #[arg(long)]
        auto_fetch_interval_hours: Option<i64>,
        #[arg(long)]
        auto_analysis_interval_hours: Option<i64>,
    },
    /// Detailed health check
    Health,
    /// Export campaign data
    Export {
        #[arg(long)]
        campaign_id: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::new();
    let base = cli.base_url.trim_end_matches('/').to_string();

    let result = run(&client, &base, cli.command).await;
    match result {
        Ok(output) => {
            // Pretty-print JSON output
            if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                println!("{}", serde_json::to_string_pretty(&parsed).unwrap_or(output));
            } else {
                println!("{}", output);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run(client: &Client, base: &str, cmd: Commands) -> anyhow::Result<String> {
    match cmd {
        Commands::Health => get(client, base, "/api/health").await,
        Commands::StartupCheck => get(client, base, "/api/system/startup-check").await,
        Commands::Serve => {
            println!("Starting backend server...");
            app_lib::server::start_server().await?;
            Ok("Server stopped".into())
        }

        Commands::Profile { action } => match action {
            ProfileAction::List => get(client, base, "/api/profiles").await,
            ProfileAction::Create { name, description, avatar_color } => {
                let body = json!({
                    "name": name,
                    "description": description,
                    "avatar_color": avatar_color,
                });
                post_json(client, base, "/api/profiles", &body).await
            }
            ProfileAction::Update { id, name, description, avatar_color } => {
                let body = json!({
                    "name": name,
                    "description": description,
                    "avatar_color": avatar_color,
                });
                put_json(client, base, &format!("/api/profiles/{}", id), &body).await
            }
            ProfileAction::Delete { id } => {
                delete(client, base, &format!("/api/profiles/{}", id)).await
            }
        },

        Commands::Product { action } => match action {
            ProductAction::List { profile_id } => {
                let qs = profile_id.map(|p| format!("?profile_id={}", p)).unwrap_or_default();
                get(client, base, &format!("/api/products{}", qs)).await
            }
            ProductAction::Create { name, r#type, description, url, price, tags, profile_id } => {
                let tags_arr = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>());
                let body = json!({
                    "name": name,
                    "type": r#type,
                    "description": description,
                    "url": url,
                    "price": price,
                    "tags": tags_arr,
                    "profile_id": profile_id,
                });
                post_json(client, base, "/api/products", &body).await
            }
            ProductAction::Update { id, name, r#type, description, url, price, tags, profile_id } => {
                let tags_arr = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>());
                let body = json!({
                    "name": name,
                    "type": r#type,
                    "description": description,
                    "url": url,
                    "price": price,
                    "tags": tags_arr,
                    "profile_id": profile_id,
                });
                put_json(client, base, &format!("/api/products/{}", id), &body).await
            }
            ProductAction::Delete { id } => {
                delete(client, base, &format!("/api/products/{}", id)).await
            }
        },

        Commands::Campaign { action } => match action {
            CampaignAction::List { profile_id } => {
                let qs = profile_id.map(|p| format!("?profile_id={}", p)).unwrap_or_default();
                get(client, base, &format!("/api/campaigns{}", qs)).await
            }
            CampaignAction::Get { id } => {
                get(client, base, &format!("/api/campaigns/{}", id)).await
            }
            CampaignAction::Create { name, product_id, goal, target_audience, start_date, end_date, notes, profile_id } => {
                let body = json!({
                    "name": name,
                    "product_id": product_id,
                    "goal": goal,
                    "target_audience": target_audience,
                    "start_date": start_date,
                    "end_date": end_date,
                    "notes": notes,
                    "profile_id": profile_id,
                });
                post_json(client, base, "/api/campaigns", &body).await
            }
            CampaignAction::Update { id, name, product_id, status, goal, target_audience, start_date, end_date, notes, profile_id } => {
                let body = json!({
                    "name": name,
                    "product_id": product_id,
                    "status": status,
                    "goal": goal,
                    "target_audience": target_audience,
                    "start_date": start_date,
                    "end_date": end_date,
                    "notes": notes,
                    "profile_id": profile_id,
                });
                put_json(client, base, &format!("/api/campaigns/{}", id), &body).await
            }
            CampaignAction::Delete { id } => {
                delete(client, base, &format!("/api/campaigns/{}", id)).await
            }
        },

        Commands::Post { action } => match action {
            PostAction::List { campaign_id } => {
                get(client, base, &format!("/api/campaigns/{}/posts", campaign_id)).await
            }
            PostAction::Create { campaign_id, platform, post_type, platform_post_id, url, title, body_preview, target_community, posted_at, tags, is_api_tracked } => {
                let tags_arr = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>());
                let body = json!({
                    "platform": platform,
                    "post_type": post_type,
                    "platform_post_id": platform_post_id,
                    "url": url,
                    "title": title,
                    "body_preview": body_preview,
                    "target_community": target_community,
                    "posted_at": posted_at,
                    "tags": tags_arr,
                    "is_api_tracked": is_api_tracked,
                });
                post_json(client, base, &format!("/api/campaigns/{}/posts", campaign_id), &body).await
            }
            PostAction::Update { id, platform, post_type, platform_post_id, url, title, body_preview, target_community, posted_at, tags, is_api_tracked } => {
                let tags_arr = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>());
                let body = json!({
                    "platform": platform,
                    "post_type": post_type,
                    "platform_post_id": platform_post_id,
                    "url": url,
                    "title": title,
                    "body_preview": body_preview,
                    "target_community": target_community,
                    "posted_at": posted_at,
                    "tags": tags_arr,
                    "is_api_tracked": is_api_tracked,
                });
                put_json(client, base, &format!("/api/posts/{}", id), &body).await
            }
            PostAction::Delete { id } => {
                delete(client, base, &format!("/api/posts/{}", id)).await
            }
        },

        Commands::Metrics { action } => match action {
            MetricsAction::Post { post_id } => {
                get(client, base, &format!("/api/posts/{}/metrics", post_id)).await
            }
            MetricsAction::Campaign { campaign_id } => {
                get(client, base, &format!("/api/campaigns/{}/metrics", campaign_id)).await
            }
            MetricsAction::Fetch => {
                post_json(client, base, "/api/metrics/fetch", &json!({})).await
            }
            MetricsAction::Status => {
                get(client, base, "/api/metrics/fetch/status").await
            }
        },

        Commands::Analytics { action } => {
            let (path, profile_id) = match action {
                AnalyticsAction::Overview { profile_id } => ("/api/analytics/overview", profile_id),
                AnalyticsAction::Platforms { profile_id } => ("/api/analytics/platforms", profile_id),
                AnalyticsAction::PostTypes { profile_id } => ("/api/analytics/post-types", profile_id),
                AnalyticsAction::Trends { profile_id } => ("/api/analytics/trends", profile_id),
            };
            let qs = profile_id.map(|p| format!("?profile_id={}", p)).unwrap_or_default();
            get(client, base, &format!("{}{}", path, qs)).await
        }

        Commands::Ai { action } => match action {
            AiAction::Latest { profile_id } => {
                let qs = profile_id.map(|p| format!("?profile_id={}", p)).unwrap_or_default();
                get(client, base, &format!("/api/ai/latest{}", qs)).await
            }
            AiAction::Campaign { campaign_id } => {
                get(client, base, &format!("/api/ai/campaign/{}", campaign_id)).await
            }
            AiAction::Trigger => {
                post_json(client, base, "/api/ai/trigger", &json!({})).await
            }
            AiAction::Status => {
                get(client, base, "/api/ai/status").await
            }
            AiAction::Recommendations => {
                get(client, base, "/api/ai/recommendations").await
            }
            AiAction::Insight => {
                get(client, base, "/api/ai/cross-campaign-insight").await
            }
            AiAction::KbQuery { q } => {
                get(client, base, &format!("/api/ai/knowledge-base/query?q={}", urlencoding(&q))).await
            }
            AiAction::KbStats => {
                get(client, base, "/api/ai/knowledge-base/stats").await
            }
        },

        Commands::Settings { action } => match action {
            SettingsAction::Get => {
                get(client, base, "/api/settings").await
            }
            SettingsAction::UpdatePlatform { platform, credentials, is_enabled, config } => {
                let creds: Option<Value> = credentials.and_then(|c| serde_json::from_str(&c).ok());
                let cfg: Option<Value> = config.and_then(|c| serde_json::from_str(&c).ok());
                let body = json!({
                    "credentials": creds,
                    "is_enabled": is_enabled,
                    "config": cfg,
                });
                put_json(client, base, &format!("/api/settings/platform/{}", platform), &body).await
            }
            SettingsAction::UpdateGeneral { data_dir, auto_fetch_interval_hours, auto_analysis_interval_hours } => {
                let body = json!({
                    "data_dir": data_dir,
                    "auto_fetch_interval_hours": auto_fetch_interval_hours,
                    "auto_analysis_interval_hours": auto_analysis_interval_hours,
                });
                put_json(client, base, "/api/settings/general", &body).await
            }
            SettingsAction::Health => {
                get(client, base, "/api/settings/health").await
            }
            SettingsAction::Export { campaign_id } => {
                post_json(client, base, &format!("/api/system/export/{}", campaign_id), &json!({})).await
            }
        },
    }
}

// ── HTTP helpers ──────────────────────────────────────────

async fn get(client: &Client, base: &str, path: &str) -> anyhow::Result<String> {
    let url = format!("{}{}", base, path);
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("HTTP {} {}: {}", status.as_u16(), path, body);
    }
    Ok(body)
}

async fn post_json(client: &Client, base: &str, path: &str, body: &Value) -> anyhow::Result<String> {
    let url = format!("{}{}", base, path);
    let resp = client.post(&url).json(body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() && !status.is_redirection() {
        anyhow::bail!("HTTP {} {}: {}", status.as_u16(), path, text);
    }
    Ok(text)
}

async fn put_json(client: &Client, base: &str, path: &str, body: &Value) -> anyhow::Result<String> {
    let url = format!("{}{}", base, path);
    let resp = client.put(&url).json(body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("HTTP {} {}: {}", status.as_u16(), path, text);
    }
    Ok(text)
}

async fn delete(client: &Client, base: &str, path: &str) -> anyhow::Result<String> {
    let url = format!("{}{}", base, path);
    let resp = client.delete(&url).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("HTTP {} {}: {}", status.as_u16(), path, text);
    }
    Ok(text)
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
}
