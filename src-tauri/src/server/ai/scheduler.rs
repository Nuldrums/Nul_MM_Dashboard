use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, interval};
use crate::server::AppState;
use crate::server::services::daily_pipeline;

pub fn start(state: Arc<AppState>) {
    tokio::spawn(async move {
        // Check every hour
        let mut tick = interval(Duration::from_secs(3600));
        tick.tick().await; // First tick is immediate, skip it
        loop {
            tick.tick().await;
            if let Err(e) = check_and_trigger(&state).await {
                tracing::error!("Scheduler check failed: {}", e);
            }
        }
    });
    tracing::info!("Background analysis scheduler started (24h cycle, hourly checks)");
}

async fn check_and_trigger(state: &Arc<AppState>) -> anyhow::Result<()> {
    if state.analysis_running.load(Ordering::Relaxed) {
        tracing::debug!("Analysis already running, skipping scheduler check");
        return Ok(());
    }

    if state.settings.anthropic_api_key.is_empty() {
        return Ok(());
    }

    let last_analysis: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'last_ai_analysis'"
    ).fetch_optional(&state.db).await?;

    let should_run = match last_analysis {
        None => true,
        Some((None,)) => true,
        Some((Some(ts),)) => {
            if let Ok(last) = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S") {
                let elapsed = chrono::Utc::now().naive_utc() - last;
                elapsed.num_hours() >= 24
            } else {
                true
            }
        }
    };

    if should_run {
        tracing::info!("Scheduler triggering daily analysis pipeline");
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = daily_pipeline::run_full(&state).await {
                tracing::error!("Daily pipeline failed: {}", e);
            }
        });
    }

    Ok(())
}
