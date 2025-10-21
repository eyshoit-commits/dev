mod config;
mod db;
mod http;
mod security;

use crate::config::Settings;
use crate::db::Database;
use crate::http::build_router;
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

    let settings = Settings::load()?;
    info!(?settings, "MAID APIkeys service starting");

    let database = Database::new(&settings.database.path)?;
    let app_state = Arc::new(http::AppState::new(settings.clone(), database.clone()));
    let router = build_router(app_state.clone());

    let addr = settings.server.socket_addr();
    info!(%addr, "starting APIkeys HTTP server");

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

    info!("APIkeys shutdown complete");
    Ok(())
}
