//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. WebSocket proxy for multiplayer connections (TODO)

use axum::{
    extract::Path,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use monitor_common::rpe; // Use the parser from monitor crate
use std::io::Cursor;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

const PHIRA_API_BASE: &str = "https://api.phira.cn";
const LISTEN_PORT: u16 = 3080;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Phira Web Monitor Proxy starting...");

    // CORS configuration - allow all origins for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        // Chart parsing endpoint: /chart/:id -> bincode
        .route("/chart/:id", get(fetch_and_parse_chart))
        // Serve static files from web/dist (for production)
        .fallback_service(ServeDir::new("../web/dist"))
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], LISTEN_PORT));
    log::info!("Listening on http://{}", addr);
    log::info!("API Base: {}", PHIRA_API_BASE);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Fetch chart info, download zip, parse RPE, and return bincode
async fn fetch_and_parse_chart(Path(id): Path<String>) -> Response {
    log::info!("Processing chart request for ID: {}", id);

    match process_chart(&id).await {
        Ok(bytes) => {
            log::info!("Chart {} parsed successfully ({} bytes)", id, bytes.len());
            let mut response = (StatusCode::OK, bytes).into_response();
            response.headers_mut().insert(
                "content-type",
                HeaderValue::from_static("application/octet-stream"),
            );
            response
        }
        Err(e) => {
            log::error!("Error processing chart {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response()
        }
    }
}

async fn process_chart(id: &str) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::Client::new();

    // 1. Get chart info
    let info_url = format!("{}/chart/{}", PHIRA_API_BASE, id);
    let info_resp = client.get(&info_url).send().await?;
    if !info_resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch chart info: {}",
            info_resp.status()
        ));
    }

    let info_json: serde_json::Value = info_resp.json().await?;
    let file_url = info_json["file"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No file URL in chart info"))?;

    log::info!("Downloading chart file from: {}", file_url);

    // 2. Download zip file
    let file_resp = client.get(file_url).send().await?;
    if !file_resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download chart file: {}",
            file_resp.status()
        ));
    }
    let zip_bytes = file_resp.bytes().await?;

    // 3. Unzip and find chart.json
    let reader = Cursor::new(zip_bytes);
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut chart_json = String::new();
    let mut found = false;
    let len = zip.len();

    for i in 0..len {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".json") && (file.name().contains("chart") || len == 1 || !found) {
            use std::io::Read;
            chart_json.clear();
            file.read_to_string(&mut chart_json)?;
            found = true;
            // Prefer file named "chart.json" or similar, but keep first json found as fallback
            if file.name().contains("chart") {
                break;
            }
        }
    }

    if !found {
        return Err(anyhow::anyhow!("No JSON chart file found in zip"));
    }

    // 4. Parse RPE chart
    let chart =
        rpe::parse_rpe(&chart_json).map_err(|e| anyhow::anyhow!("RPE parse error: {}", e))?;

    // 5. Serialize to bincode
    let encoded = bincode::serialize(&chart)?;

    Ok(encoded)
}
