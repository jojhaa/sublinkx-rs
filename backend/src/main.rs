mod api;
mod app;
mod config;
mod db;
mod domain;
mod dto;
mod errors;
mod middleware;
mod protocols;
mod repository;
mod services;
mod state;
mod utils;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing::info;

use crate::{app::build_app, config::AppConfig, db::new_database_pool, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    utils::telemetry::init_tracing();
    let _ = dotenvy::dotenv();

    let config = AppConfig::from_env()?;
    let pool = new_database_pool(&config.database.url).await?;
    repository::user_repo::bootstrap_admin(&pool, &config.security).await?;
    services::template_seed_service::seed_default_templates(&pool).await?;
    let state = AppState::new(config.clone(), pool);
    services::latency_scheduler_service::spawn_auto_latency_tester(state.clone());
    let app = build_app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    let listener = TcpListener::bind(addr).await?;

    info!("backend listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(utils::shutdown::graceful_shutdown())
        .await?;

    Ok(())
}
