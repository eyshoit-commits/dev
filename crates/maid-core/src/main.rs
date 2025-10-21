mod api;
mod auth;
mod config;
mod metrics;
mod persistence;
mod plugin;
mod runner;

use crate::api::build_router;
use crate::config::Settings;
use crate::persistence::HistoryStore;
use crate::plugin::PluginRegistry;
use crate::runner::EngineHandle;
use anyhow::Context;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let settings = Settings::load().context("failed to load settings")?;
    info!(?settings, "MAID Goose core starting");

    let history = HistoryStore::new(&settings.persistence.database_path)
        .context("failed to initialise persistence layer")?;

    let plugins = PluginRegistry::new(&settings).context("failed to initialise plugin registry")?;

    let engine = EngineHandle::new(settings.clone(), history.clone());

    let app_state = Arc::new(api::AppState::new(
        settings.clone(),
        engine,
        history,
        plugins,
    )?);

    let router = build_router(app_state.clone());

    let addr = settings.server.socket_addr();
    info!(%addr, "starting Goose core HTTP server");

    let server = axum::Server::bind(&addr).serve(router.into_make_service());

    tokio::select! {
        result = server => {
            if let Err(err) = result {
                error!(?err, "server error");
                return Err(err.into());
            }
        }
        _ = signal::ctrl_c() => {
            info!("ctrl-c received, shutting down");
        }
    }

    app_state.engine().graceful_shutdown().await;

    info!("shutdown complete");
    Ok(())
}
