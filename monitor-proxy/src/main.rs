//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. Disk-based chart caching with in-flight request deduplication

use axum::{http::Method, routing::get, routing::post, Router};
use axum_extra::extract::cookie::Key;
use clap::Parser;
use reqwest::Client;
use std::{
    collections::HashMap, env, net::SocketAddr, os::unix::ffi::OsStrExt, path::PathBuf, sync::Arc,
};
use tokio::sync::{broadcast, Mutex};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

mod auth;
mod chart;
mod rooms;

// ── CLI Arguments ──────────────────────────────────────────────────────────────

fn default_cache_path() -> PathBuf {
    let mut path: PathBuf = env::var_os("HOME").unwrap_or(".".into()).into();
    path.push(".cache");
    path.push("hsn-phira");
    path
}

#[derive(Parser, Debug, Clone)]
#[command(name = "monitor-proxy", about = "Phira Web Monitor Proxy Server")]
pub struct Args {
    /// Debug mode
    #[arg(long)]
    pub debug: bool,

    /// Port to listen on
    #[arg(long, default_value_t = 3080)]
    pub port: u16,

    /// Directory for disk-based chart cache
    #[arg(long, default_value_os_t = default_cache_path())]
    pub cache_dir: PathBuf,

    /// Phira API base URL
    #[arg(long, default_value = "https://phira.5wyxi.com")]
    pub api_base: String,

    /// Phira-mp server address
    #[arg(long, default_value = "localhost:12346")]
    pub mp_server: String,
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppStateInner {
    /// CLI arguments
    pub args: Args,
    /// http client
    pub client: Client,
    /// In-flight task deduplication: chart_id → broadcast sender.
    /// Waiters receive Ok(()) on success (then read from disk), or Err(msg) on failure.
    pub in_flight: Mutex<HashMap<String, broadcast::Sender<Result<(), String>>>>,
    /// Secret key for cookie signing
    pub key: Key,
}

pub struct AppState(Arc<AppStateInner>);

impl AppState {
    pub fn new(args: Args) -> Self {
        let key = if args.debug {
            Key::generate()
        } else {
            Key::from(
                env::var_os("HSN_SECRET_KEY")
                    .expect("environment variable HSN_SECRET_KEY not set")
                    .as_bytes(),
            )
        };
        Self(Arc::new(AppStateInner {
            args,
            client: Client::new(),
            in_flight: Mutex::new(HashMap::new()),
            key,
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

// ── Macros ─────────────────────────────────────────────────────────────────────

#[macro_export]
macro_rules! json_err {
    ($($arg: tt)*) => {
        {
            use serde_json::json;
            use axum::{Json, response::IntoResponse};
            let msg = format!($($arg)*);
            Json(json!({"error": msg})).into_response()
        }
    };
}
#[macro_export]
macro_rules! json_msg {
    ($($arg: tt)*) => {
        {
            use serde_json::json;
            use axum::{Json, response::IntoResponse};
            let msg = format!($($arg)*);
            Json(json!({"message": msg})).into_response()
        }
    };
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
    let state = AppState::new(args);

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let public_routes = Router::new()
        .route("/chart/{id}", get(chart::fetch_and_parse_chart))
        .route("/rooms", get(rooms::query_rooms))
        .route("/rooms/{id}", get(rooms::query_room))
        .route("/auth/login", post(auth::login));
    let protected_routes = Router::new()
        .route("/auth/me", get(auth::get_me_profile))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback_service(ServeDir::new("../web/dist"))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
