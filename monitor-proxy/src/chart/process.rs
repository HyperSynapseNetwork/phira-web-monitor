use super::parse::{pbc, pec, pgr, rpe, ResourceLoader};
use anyhow::Context;
use monitor_common::core::{ChartFormat, ChartInfo};
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

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
            let mut archive = archive.lock().unwrap();
            let mut file = archive.by_name(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            Ok(buffer)
        })
    }
}

/// Process a chart from the API response JSON.
/// Audio is pre-extracted from the zip BEFORE format-specific parsing,
/// so zip_bytes can safely be moved into RPE's ZipLoader.
pub async fn process_chart_from_api(
    client: &reqwest::Client,
    info_json: &serde_json::Value,
) -> anyhow::Result<Vec<u8>> {
    let file_url = info_json["file"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No file URL in chart info"))?;

    log::info!("Downloading chart file from: {}", file_url);

    // Download zip (single allocation)
    let file_resp = client.get(file_url).send().await?;
    if !file_resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download chart file: {}",
            file_resp.status()
        ));
    }
    let zip_bytes = file_resp.bytes().await?.to_vec();

    // Open zip archive — borrow, no clone
    let mut zip = zip::ZipArchive::new(Cursor::new(&zip_bytes[..]))?;

    // Read info.yml
    let mut info: ChartInfo = serde_yaml::from_reader(
        zip.by_path("info.yml")
            .with_context(|| "Cannot find info.yml in chart zip")?,
    )
    .with_context(|| "Failed to parse info.yml")?;

    // Read chart file
    let mut chart_bytes = Vec::new();
    zip.by_path(&info.chart)
        .with_context(|| "Cannot find chart file")?
        .read_to_end(&mut chart_bytes)
        .with_context(|| "Failed to read chart file")?;

    // Read extra.json (optional)
    let extra_json = zip
        .by_path("extra.json")
        .and_then(|mut file| {
            let mut s = String::new();
            file.read_to_string(&mut s)?;
            Ok(Some(s))
        })
        .unwrap_or(None);

    // Extract audio BEFORE format dispatch (while we still borrow zip_bytes)
    log::info!("Extracting audio resources...");
    let music_data = extract_file_bytes(&mut zip, &info.music);
    let hitsound_data = extract_hitsound_bytes(&mut zip, &extra_json);

    // Detect format from raw bytes (no clone needed)
    info.format = info.format.or_else(|| {
        if chart_bytes.first() == Some(&b'{') {
            if chart_bytes.windows(4).any(|w| w == b"META") {
                log::info!("Detected RPE chart");
                Some(ChartFormat::Rpe)
            } else {
                log::info!("Detected PGR chart");
                Some(ChartFormat::Pgr)
            }
        } else if chart_bytes.first().map_or(false, |b| b.is_ascii()) {
            log::info!("Detected PEC chart");
            Some(ChartFormat::Pec)
        } else {
            log::info!("Detected PBC chart");
            Some(ChartFormat::Pbc)
        }
    });

    // Drop the borrow-based zip so we can move zip_bytes if needed (RPE)
    drop(zip);

    // Parse chart
    let mut chart = match info.format.clone().unwrap() {
        ChartFormat::Rpe => {
            let chart_text = String::from_utf8(chart_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?;
            // Move zip_bytes into the RPE loader (no clone)
            let archive = Arc::new(Mutex::new(zip::ZipArchive::new(Cursor::new(zip_bytes))?));
            let mut loader = ZipLoader { archive };
            rpe::parse_rpe(&chart_text, &mut loader)
                .await
                .map_err(|e| anyhow::anyhow!("RPE parse error: {}", e))?
        }
        ChartFormat::Pgr => {
            let chart_text = String::from_utf8(chart_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?;
            pgr::parse_pgr(&chart_text)
                .await
                .map_err(|e| anyhow::anyhow!("PGR parse error: {}", e))?
        }
        ChartFormat::Pec => {
            let chart_text = String::from_utf8(chart_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?;
            pec::parse_pec(&chart_text)
                .await
                .map_err(|e| anyhow::anyhow!("PEC parse error: {}", e))?
        }
        ChartFormat::Pbc => pbc::parse_pbc(&chart_bytes)
            .await
            .map_err(|e| anyhow::anyhow!("PBC parse error: {}", e))?,
    };

    // Load audio from pre-extracted bytes
    load_audio_into_chart(&info, music_data, hitsound_data, &mut chart);

    // Serialize
    use bincode::Options;
    bincode::options()
        .with_varint_encoding()
        .serialize(&(info, chart))
        .with_context(|| "Failed to serialize chart")
}

// ── Audio Extraction Helpers ───────────────────────────────────────────────────

/// Extract raw bytes of a single file from the zip.
fn extract_file_bytes(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Option<(Vec<u8>, String)> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_string();
    let mut file = zip.by_path(path).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some((bytes, ext))
}

/// Extract hitsound files referenced in extra.json.
fn extract_hitsound_bytes(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    extra_json: &Option<String>,
) -> Vec<(String, Vec<u8>, String)> {
    let mut result = Vec::new();
    let Some(extra_source) = extra_json else {
        return result;
    };
    let Ok(extra) = super::parse::extra::parse_extra(extra_source) else {
        return result;
    };
    let Some(mappings) = extra.hitsounds else {
        return result;
    };
    for (kind_str, filename) in mappings {
        if let Ok(mut file) = zip.by_name(&filename) {
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_ok() {
                let ext = std::path::Path::new(&filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("wav")
                    .to_string();
                result.push((kind_str, bytes, ext));
            }
        }
    }
    result
}

/// Decode pre-extracted audio bytes and load them into the chart.
fn load_audio_into_chart(
    info: &ChartInfo,
    music_data: Option<(Vec<u8>, String)>,
    hitsound_data: Vec<(String, Vec<u8>, String)>,
    chart: &mut monitor_common::core::Chart,
) {
    use monitor_common::core::{AudioClip, HitSound};

    if let Some((bytes, ext)) = music_data {
        match AudioClip::load_from_bytes(&bytes, &ext) {
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

    for (kind_str, bytes, ext) in hitsound_data {
        match AudioClip::load_from_bytes(&bytes, &ext) {
            Ok(clip) => {
                let kind = match kind_str.to_lowercase().as_str() {
                    "click" => HitSound::Click,
                    "drag" => HitSound::Drag,
                    "flick" => HitSound::Flick,
                    _ => HitSound::Custom(kind_str),
                };
                chart.hitsounds.insert(kind, clip);
            }
            Err(e) => log::warn!("Failed to decode hitsound: {}", e),
        }
    }
}
