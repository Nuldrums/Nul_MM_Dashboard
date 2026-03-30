pub mod models;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use crate::server::config::Settings;

pub async fn init_pool(settings: &Settings) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(&settings.data_dir)?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;

    // Enable WAL mode and foreign keys
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;

    tracing::info!("Database initialized: {}", settings.database_url);

    create_tables(&pool).await?;
    tracing::info!("Database tables created");

    Ok(pool)
}

async fn create_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            avatar_color TEXT DEFAULT '#E8845C',
            created_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            description TEXT,
            url TEXT,
            price REAL,
            tags TEXT,
            profile_id TEXT REFERENCES profiles(id),
            created_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS campaigns (
            id TEXT PRIMARY KEY,
            product_id TEXT NOT NULL REFERENCES products(id),
            profile_id TEXT REFERENCES profiles(id),
            name TEXT NOT NULL,
            status TEXT DEFAULT 'active',
            goal TEXT,
            target_audience TEXT,
            start_date DATE,
            end_date DATE,
            notes TEXT,
            created_at DATETIME DEFAULT (datetime('now')),
            updated_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS posts (
            id TEXT PRIMARY KEY,
            campaign_id TEXT NOT NULL REFERENCES campaigns(id),
            platform TEXT NOT NULL,
            post_type TEXT NOT NULL,
            platform_post_id TEXT,
            url TEXT,
            title TEXT,
            body_preview TEXT,
            target_community TEXT,
            posted_at DATETIME,
            tags TEXT,
            is_api_tracked INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS metric_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL REFERENCES posts(id),
            snapshot_date DATE NOT NULL,
            views INTEGER DEFAULT 0,
            impressions INTEGER DEFAULT 0,
            likes INTEGER DEFAULT 0,
            dislikes INTEGER DEFAULT 0,
            comments INTEGER DEFAULT 0,
            shares INTEGER DEFAULT 0,
            saves INTEGER DEFAULT 0,
            clicks INTEGER DEFAULT 0,
            watch_time_seconds INTEGER,
            followers_gained INTEGER DEFAULT 0,
            custom_metrics TEXT,
            fetched_via TEXT DEFAULT 'manual',
            created_at DATETIME DEFAULT (datetime('now')),
            UNIQUE(post_id, snapshot_date)
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_analyses (
            id TEXT PRIMARY KEY,
            campaign_id TEXT REFERENCES campaigns(id),
            analysis_type TEXT NOT NULL,
            summary TEXT NOT NULL,
            top_performers TEXT,
            underperformers TEXT,
            patterns TEXT,
            recommendations TEXT,
            raw_response TEXT,
            model_used TEXT,
            tokens_used INTEGER,
            analyzed_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS system_state (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS platform_configs (
            platform TEXT PRIMARY KEY,
            credentials TEXT,
            is_enabled INTEGER DEFAULT 0,
            rate_limit_remaining INTEGER,
            last_fetched_at DATETIME,
            config TEXT
        )"
    ).execute(pool).await?;

    // FTS5 table for knowledge base (replaces ChromaDB)
    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
            doc_id,
            doc_type,
            content,
            metadata
        )"
    ).execute(pool).await?;

    Ok(())
}

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    // Ensure system_state table exists for version tracking
    let version: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'schema_version'"
    ).fetch_optional(pool).await?;

    let current: i32 = version
        .and_then(|v| v.0.parse().ok())
        .unwrap_or(1);

    const SCHEMA_VERSION: i32 = 2;

    if current < SCHEMA_VERSION {
        tracing::info!("Running migrations: v{} -> v{}", current, SCHEMA_VERSION);

        if current < 2 {
            tracing::info!("Applying migration v1 -> v2: adding profiles support");
            // Add profile_id columns if they don't exist
            // SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we check first
            let has_col: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name='profile_id'"
            ).fetch_one(pool).await? > 0;

            if !has_col {
                sqlx::query("ALTER TABLE products ADD COLUMN profile_id TEXT REFERENCES profiles(id)")
                    .execute(pool).await?;
            }

            let has_col: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM pragma_table_info('campaigns') WHERE name='profile_id'"
            ).fetch_one(pool).await? > 0;

            if !has_col {
                sqlx::query("ALTER TABLE campaigns ADD COLUMN profile_id TEXT REFERENCES profiles(id)")
                    .execute(pool).await?;
            }
        }

        sqlx::query(
            "INSERT INTO system_state (key, value, updated_at) VALUES ('schema_version', ?1, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = ?1, updated_at = datetime('now')"
        ).bind(SCHEMA_VERSION.to_string()).execute(pool).await?;

        tracing::info!("Migrations complete, now at schema v{}", SCHEMA_VERSION);
    } else {
        tracing::info!("Schema is up to date (v{})", current);
    }

    Ok(())
}
