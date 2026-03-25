//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. Disk-based chart caching with in-flight request deduplication

use clap::Parser;
use log::{error, info};
use migration::{Migrator, MigratorTrait};
use reqwest::Client;
use sea_orm::{Database, DatabaseConnection};
use std::{net::SocketAddr, ops::Deref, sync::Arc};

mod config;
mod dtos;
mod entity;
mod error;
mod handlers;
mod middlewares;
mod router;
mod services;
mod utils;

pub struct AppStateInner {
    pub config: config::Config,
    pub db: DatabaseConnection,
    pub http_client: Client,
    pub auth_service: services::AuthService,
    pub chart_service: services::ChartService,
    pub room_service: services::RoomService,
    pub live_service: services::LiveService,
}

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

impl AppState {
    pub async fn new(config: config::Config) -> Self {
        let db = Database::connect(&config.database_url)
            .await
            .expect("Failed to connect to database");

        Migrator::up(&db, None)
            .await
            .expect("Failed to run database migrations");

        info!("Database connected and migrations applied.");

        let room_service = services::RoomService::new(&config.mp_server)
            .await
            .inspect_err(|e| error!("failed to setup RoomService: {e}"))
            .unwrap();

        Self(Arc::new(AppStateInner {
            config,
            db,
            http_client: Client::new(),
            auth_service: services::AuthService::new(),
            chart_service: services::ChartService::new(),
            room_service,
            live_service: services::LiveService::new(),
        }))
    }
}

impl Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = config::Config::parse();
    info!("Phira Web Monitor Proxy starting...");
    info!("API Base: {}", config.api_base);
    info!("Cache Dir: {:?}", config.cache_dir);

    let state = AppState::new(config).await;
    let addr = SocketAddr::new(state.config.host, state.config.port);
    let app = router::init_router(state);

    log::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
