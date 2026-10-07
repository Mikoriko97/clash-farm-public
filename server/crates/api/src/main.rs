//! `cf-api` binary: config from env, connect, serve with graceful shutdown.
#![forbid(unsafe_code)]

use cf_api::{build_app, AppState};
use cf_common::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let bind = config.bind;
    let redis_url = config.redis_url.expose().to_owned();
    let state = AppState::connect(config).await?;
    // Presence across api instances: Redis pub/sub bridge (docs/MAINLAND.md §8).
    tokio::spawn(state.presence.clone().run_bridge(redis::Client::open(redis_url)?));
    // Background jobs (RUNBOOK §Jobs); CF_JOBS_DISABLED=1 keeps a throwaway instance quiet.
    if std::env::var("CF_JOBS_DISABLED").is_ok_and(|v| matches!(v.as_str(), "1" | "true")) {
        tracing::warn!("background jobs disabled by CF_JOBS_DISABLED");
    } else {
        cf_api::jobs::spawn_all(state.clone());
    }
    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "cf-api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::warn!("ctrl_c handler unavailable; running until killed");
        std::future::pending::<()>().await;
    }
    tracing::info!("shutdown signal received");
}
