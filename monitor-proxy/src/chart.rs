mod cache;
pub(crate) mod parse;
mod process;
mod test_chart;

use crate::{json_err, AppState};
use anyhow::Result;
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};
use reqwest::header;
use std::path::PathBuf;
use tokio_util::io::ReaderStream;

pub async fn fetch_and_parse_chart(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Response) {
    log::info!("Processing chart request for ID: {id}");

    match handle_chart_request(&state, &id).await {
        Ok(bin_path) => {
            log::info!("Chart {id} ready, streaming from {bin_path:?}");
            match tokio::fs::File::open(&bin_path).await {
                Ok(file) => {
                    let stream = ReaderStream::new(file);
                    (
                        StatusCode::OK,
                        Response::builder()
                            .header(header::CONTENT_TYPE, "application/octet-stream")
                            .body(Body::from_stream(stream))
                            .unwrap(),
                    )
                }
                Err(e) => {
                    log::error!("Failed to open cached file {bin_path:?}: {e}");
                    (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}"))
                }
            }
        }
        Err(e) => {
            log::error!("Error processing chart {id}: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, json_err!("{e}"))
        }
    }
}

async fn handle_chart_request(state: &AppState, id: &str) -> Result<PathBuf> {
    // Test chart — write through cache so it also streams from disk
    if id == "test" {
        log::info!("Generating test chart...");
        let (info, chart) = test_chart::generate_test_chart()?;
        // Use a fixed "test" entry with empty chart_updated (always regenerates)
        let guard = match cache::acquire(&state.args.cache_dir, "test", "").await? {
            cache::CacheResult::Hit(path) => return Ok(path),
            cache::CacheResult::Miss(guard) => guard,
        };
        return guard.write("", &(info, chart));
    }

    // 1. Fetch metadata (cheap, ~1KB) to get chartUpdated
    let info_url = format!("{}/chart/{}", state.args.api_base, id);
    let info_resp = state.http_client.get(&info_url).send().await?;
    if !info_resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch chart info: {}",
            info_resp.status()
        ));
    }
    let info_json: serde_json::Value = info_resp.json().await?;
    let chart_updated = info_json["chartUpdated"].as_str().unwrap_or("").to_string();

    // 2. Acquire cache lock — blocks if another request is already downloading this chart
    match cache::acquire(&state.args.cache_dir, id, &chart_updated).await? {
        cache::CacheResult::Hit(path) => {
            log::info!("Chart {} served from cache", id);
            Ok(path)
        }
        cache::CacheResult::Miss(guard) => {
            log::info!("Chart {} cache miss, downloading...", id);
            let (info, chart) =
                process::process_chart_from_api(&state.http_client, &info_json).await?;
            let path = guard.write(&chart_updated, &(info, chart))?;
            log::info!("Chart {} cached to disk", id);
            Ok(path)
        }
    }
}
