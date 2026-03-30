pub mod config;
pub mod db;
pub mod error;
pub mod routes;
pub mod ai;
pub mod connectors;
pub mod services;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use axum::Router;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub settings: config::Settings,
    pub http_client: reqwest::Client,
    pub analysis_running: AtomicBool,
    pub fetch_running: AtomicBool,
}

pub async fn start_server() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let settings = config::Settings::load()?;
    tracing::info!("Data dir: {}", settings.data_dir);

    let pool = db::init_pool(&settings).await?;
    db::run_migrations(&pool).await?;

    let port = settings.api_port;

    let state = Arc::new(AppState {
        db: pool,
        settings,
        http_client: reqwest::Client::new(),
        analysis_running: AtomicBool::new(false),
        fetch_running: AtomicBool::new(false),
    });

    // Start background scheduler
    ai::scheduler::start(state.clone());

    let app = Router::new()
        .merge(routes::build_routes())
        .layer(CorsLayer::very_permissive())
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    tracing::info!("Backend server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
