use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    pub data_dir: String,
    pub database_url: String,
    pub api_port: u16,
    pub cors_origins: Vec<String>,
    pub anthropic_api_key: String,
    pub reddit_client_id: String,
    pub reddit_client_secret: String,
    pub reddit_username: String,
    pub reddit_password: String,
    pub youtube_api_key: String,
    pub twitter_bearer_token: String,
    pub discord_bot_token: String,
}

impl Settings {
    /// Load settings before tracing is initialized (no log calls).
    pub fn load_early() -> anyhow::Result<Self> {
        let env_paths = Self::env_search_paths();
        for path in &env_paths {
            if path.exists() {
                let _ = dotenvy::from_path(path);
                break;
            }
        }
        Self::build()
    }

    fn build() -> anyhow::Result<Self> {
        let data_dir = Self::resolve_data_dir();
        std::fs::create_dir_all(&data_dir)?;

        // Use legacy DB name if it exists, otherwise new name
        let legacy_db = PathBuf::from(&data_dir).join("trikeri.db");
        let db_path = if legacy_db.exists() {
            legacy_db
        } else {
            PathBuf::from(&data_dir).join("meem.db")
        };
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());

        Ok(Settings {
            data_dir,
            database_url,
            api_port: env_or("API_PORT", "31415").parse().unwrap_or(31415),
            cors_origins: vec!["*".into()],
            anthropic_api_key: env_or("ANTHROPIC_API_KEY", ""),
            reddit_client_id: env_or("REDDIT_CLIENT_ID", ""),
            reddit_client_secret: env_or("REDDIT_CLIENT_SECRET", ""),
            reddit_username: env_or("REDDIT_USERNAME", ""),
            reddit_password: env_or("REDDIT_PASSWORD", ""),
            youtube_api_key: env_or("YOUTUBE_API_KEY", ""),
            twitter_bearer_token: env_or("TWITTER_BEARER_TOKEN", ""),
            discord_bot_token: env_or("DISCORD_BOT_TOKEN", ""),
        })
    }

    fn resolve_data_dir() -> String {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let base = PathBuf::from(appdata);
            // Check legacy location first to preserve existing data
            let legacy = base.join("TrikeriMarketingEngine").join("data");
            if legacy.exists() {
                return legacy.to_string_lossy().into_owned();
            }
            let p = base.join("MEEM Marketing").join("data");
            return p.to_string_lossy().into_owned();
        }
        // Fallback for dev
        String::from("backend/data")
    }

    fn env_search_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Next to the exe
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                paths.push(dir.join(".env"));
            }
        }

        // AppData
        if let Some(appdata) = std::env::var_os("APPDATA") {
            paths.push(PathBuf::from(appdata.clone()).join("MEEM Marketing").join(".env"));
            // Also check legacy location
            paths.push(PathBuf::from(appdata).join("TrikeriMarketingEngine").join(".env"));
        }

        // CWD
        paths.push(PathBuf::from(".env"));

        // Backend subdir (dev mode)
        paths.push(PathBuf::from("backend").join(".env"));

        paths
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
