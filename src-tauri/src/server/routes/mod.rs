pub mod profiles;
pub mod products;
pub mod campaigns;
pub mod posts;
pub mod metrics;
pub mod analytics;
pub mod ai_analysis;
pub mod settings;

use std::sync::Arc;
use axum::{Router, Json, routing::get};
use serde_json::json;
use chrono::Utc;
use crate::server::AppState;

pub fn build_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/health", get(health_check))
        .merge(profiles::routes())
        .merge(products::routes())
        .merge(campaigns::routes())
        .merge(posts::routes())
        .merge(metrics::routes())
        .merge(analytics::routes())
        .merge(ai_analysis::routes())
        .merge(settings::routes())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339(),
        "version": "0.1.0-rust"
    }))
}
