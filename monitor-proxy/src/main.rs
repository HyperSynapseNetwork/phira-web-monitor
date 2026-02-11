//! Phira Web Monitor - Proxy Server
//!
//! This server provides:
//! 1. Static file serving for the web frontend
//! 2. Server-side chart parsing (download -> unzip -> parse -> bincode)
//! 3. WebSocket proxy for multiplayer connections (TODO)

use anyhow::Context;
use axum::{
    extract::Path,
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use monitor_common::core::{ChartFormat, ChartInfo};
use parse::{pbc, pec, pgr, rpe, ResourceLoader};
use std::io::{Cursor, Read};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

mod parse;

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

struct ZipLoader {
    archive: Arc<Mutex<zip::ZipArchive<Cursor<Vec<u8>>>>>,
}

impl ResourceLoader for ZipLoader {
    fn load_file<'a>(
        &'a mut self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>>
    {
        let archive = self.archive.clone();
        let path = path.to_string();
        Box::pin(async move {
            // Memory operations are fast enough to be synchronous here
            let mut archive = archive.lock().unwrap();
            let mut file = archive.by_name(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            Ok(buffer)
        })
    }
}

async fn process_chart(id: &str) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::Client::new();

    // 1. Get chart info
    if id == "test" {
        log::info!("Generating test chart...");
        return generate_test_chart();
    }

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
    let zip_bytes = file_resp.bytes().await?.to_vec();

    // 3. Unzip and find chart file
    let reader = Cursor::new(zip_bytes.clone());
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut info: ChartInfo = serde_yaml::from_reader(
        zip.by_path("info.yml")
            .with_context(|| "Cannot find info.yml in chart zip")?,
    )
    .with_context(|| "Failed to parse info.yml")?;

    let mut chart_bytes = Vec::new();
    zip.by_path(&info.chart)
        .with_context(|| "Cannot find chart file")?
        .read_to_end(&mut chart_bytes)
        .with_context(|| "Failed to read chart file")?;

    let extra_json = zip
        .by_path("extra.json")
        .and_then(|mut file| {
            let mut s = String::new();
            file.read_to_string(&mut s)?;
            Ok(Some(s))
        })
        .unwrap_or(None);

    // 4. Detect format and parse
    let chart_text = String::from_utf8(chart_bytes.clone());
    info.format = info.format.or_else(|| {
        if let Ok(text) = &chart_text {
            if text.starts_with("{") {
                if text.contains("META") {
                    log::info!("Detected RPE chart");
                    Some(ChartFormat::Rpe)
                } else {
                    log::info!("Detected PGR chart");
                    Some(ChartFormat::Pgr)
                }
            } else {
                log::info!("Detected PEC chart");
                Some(ChartFormat::Pec)
            }
        } else {
            log::info!("Detected PBC chart");
            Some(ChartFormat::Pbc)
        }
    });

    let mut chart = match info.format.clone().unwrap() {
        ChartFormat::Rpe => {
            let reader = Cursor::new(zip_bytes.clone());
            let archive = Arc::new(Mutex::new(zip::ZipArchive::new(reader)?));
            let mut loader = ZipLoader { archive };
            rpe::parse_rpe(&chart_text?, &mut loader)
                .await
                .map_err(|e| anyhow::anyhow!("RPE parse error: {}", e))?
        }
        ChartFormat::Pgr => pgr::parse_pgr(&chart_text?)
            .await
            .map_err(|e| anyhow::anyhow!("PGR parse error: {}", e))?,
        ChartFormat::Pec => pec::parse_pec(&chart_text?)
            .await
            .map_err(|e| anyhow::anyhow!("PEC parse error: {}", e))?,
        ChartFormat::Pbc => pbc::parse_pbc(&chart_bytes)
            .await
            .map_err(|e| anyhow::anyhow!("PBC parse error: {}", e))?,
    };

    // 5. Load Audio (Music and Hitsounds)
    log::info!("Extracting audio resources...");

    // Music
    if let Ok(mut music_file) = zip.by_path(&info.music) {
        let mut bytes = Vec::new();
        music_file.read_to_end(&mut bytes)?;
        let ext = std::path::Path::new(&info.music)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp3");

        use monitor_common::core::AudioClip;
        match AudioClip::load_from_bytes(&bytes, ext) {
            Ok(clip) => {
                log::info!(
                    "Music Loaded: {} Hz, {} channels",
                    clip.sample_rate,
                    clip.channel_count
                );
                chart.music = Some(clip);
            }
            Err(e) => log::warn!("Failed to decode music {}: {}", info.music, e),
        }
    }

    // Extra Hitsounds
    if let Some(extra_source) = extra_json {
        if let Ok(extra) = parse::extra::parse_extra(&extra_source) {
            if let Some(mappings) = extra.hitsounds {
                for (kind_str, filename) in mappings {
                    if let Ok(mut file) = zip.by_name(&filename) {
                        let mut bytes = Vec::new();
                        file.read_to_end(&mut bytes)?;
                        let ext = std::path::Path::new(&filename)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("wav");

                        use monitor_common::core::{AudioClip, HitSound};
                        match AudioClip::load_from_bytes(&bytes, ext) {
                            Ok(clip) => {
                                let kind = match kind_str.to_lowercase().as_str() {
                                    "click" => HitSound::Click,
                                    "drag" => HitSound::Drag,
                                    "flick" => HitSound::Flick,
                                    _ => HitSound::Custom(kind_str),
                                };
                                chart.hitsounds.insert(kind, clip);
                            }
                            Err(e) => {
                                log::warn!("Failed to decode custom hitsound {}: {}", filename, e)
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. Serialize to bincode
    use bincode::Options;
    bincode::options()
        .with_varint_encoding()
        .serialize(&(info, chart))
        .with_context(|| "Failed to serialize chart")
}

fn generate_test_chart() -> anyhow::Result<Vec<u8>> {
    use monitor_common::core::{AnimFloat, Chart, JudgeLine, Keyframe, Note, NoteKind};

    let mut line = JudgeLine::default();

    // Normalized height per second (e.g. 1.0 unit per second)
    const HEIGHT_PER_SEC: f32 = 1.0;

    line.height = AnimFloat::new(vec![
        Keyframe::new(0.0, 0.0, 2), // 2 = Linear
        Keyframe::new(100.0, 100.0 * HEIGHT_PER_SEC, 0),
    ]);

    // Helper to set note at time
    let mut add_note = |kind: NoteKind, time: f32| {
        let h = time * HEIGHT_PER_SEC;
        line.notes.push(Note {
            kind,
            time,
            height: h,
            speed: 1.0,
            ..Default::default()
        });
    };

    // Add some notes
    add_note(NoteKind::Click, 2.0);
    add_note(NoteKind::Click, 3.0);
    add_note(NoteKind::Drag, 3.5);
    add_note(NoteKind::Flick, 4.0);

    let start_t = 5.0;
    let end_t = 7.0;
    line.notes.push(Note {
        kind: NoteKind::Hold {
            end_time: end_t,
            end_height: end_t * HEIGHT_PER_SEC,
        },
        time: start_t,
        height: start_t * HEIGHT_PER_SEC,
        speed: 1.0,
        ..Default::default()
    });

    let info = ChartInfo::default();
    let chart = Chart {
        offset: 0.0,
        lines: vec![line],
        ..Default::default()
    };

    use bincode::Options;
    let encoded = bincode::options()
        .with_varint_encoding()
        .serialize(&(info, chart))?;
    Ok(encoded)
}
