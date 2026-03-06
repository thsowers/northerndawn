mod api;
mod config;
mod db;
mod models;
mod noaa;
mod notifications;
mod polling;
mod state;
mod viewline;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "northerndawn=info".into()),
        )
        .init();

    info!("Loading configuration...");
    let config = config::load_config()?;

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!(location = %config.location.name, "Starting Sunrise Winds");

    let state = state::AppState::new(config)?;

    // Spawn background polling tasks
    polling::spawn_polling_tasks(state.clone());

    // Build API router
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Serve frontend static files if the dist directory exists
    let static_dir = PathBuf::from("../frontend/dist");
    let app = if static_dir.exists() {
        info!(?static_dir, "Serving frontend static files");
        let serve_dir = ServeDir::new(&static_dir)
            .not_found_service(ServeFile::new(static_dir.join("index.html")));
        api::router()
            .fallback_service(serve_dir)
            .layer(cors)
            .with_state(state)
    } else {
        info!("No frontend dist found, API-only mode");
        api::router().layer(cors).with_state(state)
    };

    info!(%addr, "Server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
