//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. Disk-based chart caching with in-flight request deduplication

use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    routing::post,
    Router,
};
use clap::Parser;
use jsonwebtoken::{DecodingKey, EncodingKey};
use phira_mp_common::generate_secret_key;
use reqwest::Client;
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::{cors::CorsLayer, services::ServeDir};

mod auth;
mod chart;
mod live;
mod rooms;
mod utils;

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

    /// Allowed CORS origin (used when --debug is not set)
    #[arg(long)]
    pub allowed_origin: Option<String>,
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppStateInner {
    /// CLI arguments
    pub args: Args,

    /// HTTP client
    pub http_client: Client,

    /// Room monitor client
    pub room_monitor_client: rooms::RoomMonitorClient,

    /// Secret key for JWT encoding/decoding
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

pub struct AppState(Arc<AppStateInner>);

impl AppState {
    pub async fn new(args: Args) -> Self {
        let jwt_key = generate_secret_key("jwt", 64).expect("failed to generate key for cookie");
        let http_client = Client::new();
        let room_monitor_client = rooms::RoomMonitorClient::new(&args.mp_server)
            .await
            .expect("failed to create RoomMonitorClient");

        Self(Arc::new(AppStateInner {
            args,
            http_client,
            room_monitor_client,
            encoding_key: EncodingKey::from_secret(&jwt_key),
            decoding_key: DecodingKey::from_secret(&jwt_key),
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
    let state = AppState::new(args).await;

    // CORS configuration
    let cors = if state.args.debug {
        // Debug: mirror the request Origin header back
        // (Any + allow_credentials is forbidden by browsers and panics in tower-http)
        CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    } else {
        let origin: HeaderValue = state
            .args
            .allowed_origin
            .as_ref()
            .expect("--allowed-origin must be set")
            .parse()
            .expect("invalid --allowed-origin value");
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    };

    let app = Router::new()
        .route("/chart/{id}", get(chart::fetch_and_parse_chart))
        .route("/rooms/info", get(rooms::get_room_list))
        .route("/rooms/info/{id}", get(rooms::get_room_by_id))
        .route("/rooms/user/{id}", get(rooms::get_room_of_user))
        .route("/rooms/listen", get(rooms::listen))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::get_me_profile))
        .route("/ws/live", get(live::live_ws))
        .fallback_service(ServeDir::new("../web/dist"))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
