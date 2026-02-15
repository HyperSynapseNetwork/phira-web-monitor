mod cache;
pub(crate) mod parse;
mod process;
mod test_chart;

use crate::AppState;
use anyhow::Context;
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::header;

use tokio::sync::broadcast;

pub async fn fetch_and_parse_chart(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    log::info!("Processing chart request for ID: {}", id);

    match handle_chart_request(&state, &id).await {
        Ok(bytes) => {
            log::info!("Chart {} ready ({} bytes)", id, bytes.len());
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            log::error!("Error processing chart {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response()
        }
    }
}

async fn handle_chart_request(state: &AppState, id: &str) -> anyhow::Result<Vec<u8>> {
    // Test chart bypasses everything
    if id == "test" {
        log::info!("Generating test chart...");
        return test_chart::generate_test_chart();
    }

    // 1. Always fetch metadata (cheap, ~1KB) to get chartUpdated
    let info_url = format!("{}/chart/{}", state.args.api_base, id);
    let info_resp = state.client.get(&info_url).send().await?;
    if !info_resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch chart info: {}",
            info_resp.status()
        ));
    }
    let info_json: serde_json::Value = info_resp.json().await?;
    let chart_updated = info_json["chartUpdated"].as_str().unwrap_or("").to_string();

    // 2. Check disk cache
    if let Some(data) = cache::check(&state.args.cache_dir, id, &chart_updated) {
        log::info!("Chart {} served from disk cache", id);
        return Ok(data);
    }

    // 3. Check in-flight tasks / register ourselves
    {
        let mut in_flight = state.in_flight.lock().await;
        if let Some(tx) = in_flight.get(id) {
            // Someone else is already downloading this chart — wait for them
            let mut rx = tx.subscribe();
            drop(in_flight);
            log::info!("Chart {} waiting for in-flight task", id);
            match rx.recv().await {
                Ok(Ok(())) => {
                    // Task completed, read from disk
                    return std::fs::read(cache::bin_path(&state.args.cache_dir, id))
                        .with_context(|| "Failed to read cached result after in-flight wait");
                }
                Ok(Err(e)) => return Err(anyhow::anyhow!("In-flight task failed: {}", e)),
                Err(e) => return Err(anyhow::anyhow!("Broadcast channel error: {}", e)),
            }
        }

        // Register ourselves as the in-flight task
        let (tx, _) = broadcast::channel(16);
        in_flight.insert(id.to_string(), tx);
    }

    // 4. Download, parse, serialize — we are the worker
    let result = process::process_chart_from_api(&state.client, &info_json).await;

    // 5. Store or broadcast error, then clean up in-flight entry
    let tx = {
        let mut in_flight = state.in_flight.lock().await;
        in_flight.remove(id)
    };

    match &result {
        Ok(data) => {
            if let Err(e) = cache::write(&state.args.cache_dir, id, &chart_updated, data) {
                log::warn!("Failed to write disk cache for chart {}: {}", id, e);
            } else {
                log::info!("Chart {} cached to disk", id);
            }
            if let Some(tx) = tx {
                let _ = tx.send(Ok(()));
            }
        }
        Err(e) => {
            if let Some(tx) = tx {
                let _ = tx.send(Err(e.to_string()));
            }
        }
    }

    result
}
