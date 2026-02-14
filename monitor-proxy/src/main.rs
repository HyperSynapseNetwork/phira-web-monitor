//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. Disk-based chart caching with in-flight request deduplication

use axum::{http::Method, routing::get, Router};
use clap::Parser;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

mod chart;
mod rooms;

// ── CLI Arguments ──────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "monitor-proxy", about = "Phira Web Monitor Proxy Server")]
pub struct Args {
    /// Port to listen on
    #[arg(long, default_value_t = 3080)]
    port: u16,

    /// Directory for disk-based chart cache
    #[arg(long, default_value = "./cache")]
    pub cache_dir: PathBuf,

    /// Phira API base URL
    #[arg(long, default_value = "https://api.phira.cn")]
    pub api_base: String,

    /// Phira-mp server address
    #[arg(long, default_value = "localhost:12346")]
    pub mp_server: String,
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppState {
    pub args: Args,
    /// In-flight task deduplication: chart_id → broadcast sender.
    /// Waiters receive Ok(()) on success (then read from disk), or Err(msg) on failure.
    pub in_flight: tokio::sync::Mutex<HashMap<String, broadcast::Sender<Result<(), String>>>>,
}

// ── Main ───────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    log::info!("Phira Web Monitor Proxy starting...");
    log::info!("API Base: {}", args.api_base);
    log::info!("Cache Dir: {:?}", args.cache_dir);

    let port = args.port;
    let state = Arc::new(AppState {
        args,
        in_flight: tokio::sync::Mutex::new(HashMap::new()),
    });

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/chart/{id}", get(chart::fetch_and_parse_chart))
        .route("/rooms", get(rooms::query_rooms))
        .route("/rooms/{id}", get(rooms::query_room))
        .fallback_service(ServeDir::new("../web/dist"))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
