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
            tags TEXT,
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

    // FTS5 table for knowledge base (legacy, kept for backward compat)
    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
            doc_id,
            doc_type,
            content,
            metadata
        )"
    ).execute(pool).await?;

    // Vector knowledge base (semantic search via ONNX embeddings)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS knowledge_vectors (
            id TEXT PRIMARY KEY,
            doc_type TEXT NOT NULL,
            content TEXT NOT NULL,
            embedding BLOB NOT NULL,
            metadata TEXT,
            campaign_id TEXT,
            created_at DATETIME DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_kv_doc_type ON knowledge_vectors(doc_type)"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_kv_campaign_id ON knowledge_vectors(campaign_id)"
    ).execute(pool).await?;

    // Metric snapshots at time of analysis (for delta computation)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS analysis_metric_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            analysis_id TEXT NOT NULL,
            post_id TEXT NOT NULL,
            views INTEGER DEFAULT 0,
            likes INTEGER DEFAULT 0,
            comments INTEGER DEFAULT 0,
            shares INTEGER DEFAULT 0,
            saves INTEGER DEFAULT 0,
            clicks INTEGER DEFAULT 0,
            UNIQUE(analysis_id, post_id)
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

    const SCHEMA_VERSION: i32 = 4;

    if current < SCHEMA_VERSION {
        tracing::info!("Running migrations: v{} -> v{}", current, SCHEMA_VERSION);

        if current < 2 {
            tracing::info!("Applying migration v1 -> v2: adding profiles support");
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

        if current < 3 {
            tracing::info!("Applying migration v2 -> v3: vector knowledge base + delta analysis");

            // Add meta_learning column to ai_analyses
            let has_col: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM pragma_table_info('ai_analyses') WHERE name='meta_learning'"
            ).fetch_one(pool).await? > 0;

            if !has_col {
                sqlx::query("ALTER TABLE ai_analyses ADD COLUMN meta_learning TEXT")
                    .execute(pool).await?;
            }

            // Add metrics_hash column to ai_analyses
            let has_col: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM pragma_table_info('ai_analyses') WHERE name='metrics_hash'"
            ).fetch_one(pool).await? > 0;

            if !has_col {
                sqlx::query("ALTER TABLE ai_analyses ADD COLUMN metrics_hash TEXT")
                    .execute(pool).await?;
            }
        }

        if current < 4 {
            tracing::info!("Applying migration v3 -> v4: campaign tags column");

            let has_col: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM pragma_table_info('campaigns') WHERE name='tags'"
            ).fetch_one(pool).await? > 0;

            if !has_col {
                sqlx::query("ALTER TABLE campaigns ADD COLUMN tags TEXT")
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory pool");
        sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_create_tables() {
        let pool = test_pool().await;
        create_tables(&pool).await.expect("create_tables failed");

        // Verify core tables exist
        let tables = vec![
            "profiles", "products", "campaigns", "posts",
            "metric_snapshots", "ai_analyses", "system_state",
            "platform_configs", "knowledge_vectors", "analysis_metric_snapshots",
        ];
        for table in &tables {
            let result: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
                .fetch_one(&pool).await
                .unwrap_or_else(|e| panic!("Table '{}' should exist: {}", table, e));
            assert_eq!(result.0, 0, "Table {} should be empty", table);
        }
    }

    #[tokio::test]
    async fn test_knowledge_vectors_crud() {
        let pool = test_pool().await;
        create_tables(&pool).await.unwrap();

        let fake_embedding: Vec<u8> = vec![0u8; 384 * 4];
        sqlx::query(
            "INSERT INTO knowledge_vectors (id, doc_type, content, embedding, metadata, campaign_id)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
            .bind("doc_1").bind("pattern").bind("Test content")
            .bind(&fake_embedding).bind("{}").bind("camp_1")
            .execute(&pool).await.unwrap();

        let row: (String, String, String, Vec<u8>, Option<String>) = sqlx::query_as(
            "SELECT id, doc_type, content, embedding, campaign_id FROM knowledge_vectors WHERE id = ?"
        ).bind("doc_1").fetch_one(&pool).await.unwrap();

        assert_eq!(row.0, "doc_1");
        assert_eq!(row.1, "pattern");
        assert_eq!(row.2, "Test content");
        assert_eq!(row.3.len(), 384 * 4);
        assert_eq!(row.4.as_deref(), Some("camp_1"));
    }

    #[tokio::test]
    async fn test_analysis_metric_snapshots_crud() {
        let pool = test_pool().await;
        create_tables(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO analysis_metric_snapshots (analysis_id, post_id, views, likes, comments, shares, saves, clicks)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
            .bind("analysis_1").bind("post_1")
            .bind(1000i64).bind(50i64).bind(10i64).bind(5i64).bind(3i64).bind(20i64)
            .execute(&pool).await.unwrap();

        let row: (String, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT post_id, views, likes, comments, shares FROM analysis_metric_snapshots WHERE analysis_id = ?"
        ).bind("analysis_1").fetch_one(&pool).await.unwrap();

        assert_eq!(row.0, "post_1");
        assert_eq!(row.1, 1000);
        assert_eq!(row.2, 50);
    }

    #[tokio::test]
    async fn test_analysis_metric_snapshots_unique_constraint() {
        let pool = test_pool().await;
        create_tables(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO analysis_metric_snapshots (analysis_id, post_id, views, likes, comments, shares, saves, clicks)
             VALUES ('a1', 'p1', 100, 10, 5, 2, 1, 0)"
        ).execute(&pool).await.unwrap();

        // Duplicate should fail
        let result = sqlx::query(
            "INSERT INTO analysis_metric_snapshots (analysis_id, post_id, views, likes, comments, shares, saves, clicks)
             VALUES ('a1', 'p1', 200, 20, 10, 4, 2, 0)"
        ).execute(&pool).await;

        assert!(result.is_err(), "Duplicate (analysis_id, post_id) should violate unique constraint");
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let pool = test_pool().await;
        create_tables(&pool).await.unwrap();

        // Run migrations twice — should not fail
        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let version: (String,) = sqlx::query_as(
            "SELECT value FROM system_state WHERE key = 'schema_version'"
        ).fetch_one(&pool).await.unwrap();

        assert_eq!(version.0, "4");
    }
}
