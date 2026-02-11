use std::path::{Path, PathBuf};

#[derive(serde::Deserialize, serde::Serialize)]
struct CacheMeta {
    chart_updated: String,
}

pub fn meta_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join(format!("{}.meta", id))
}

pub fn bin_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join(format!("{}.bin", id))
}

/// Check if the disk cache has a valid entry for this chart.
pub fn check(cache_dir: &Path, id: &str, chart_updated: &str) -> Option<Vec<u8>> {
    let meta_p = meta_path(cache_dir, id);
    let bin_p = bin_path(cache_dir, id);

    let meta_bytes = std::fs::read(&meta_p).ok()?;
    let meta: CacheMeta = serde_json::from_slice(&meta_bytes).ok()?;

    if meta.chart_updated != chart_updated {
        return None;
    }

    std::fs::read(&bin_p).ok()
}

/// Write the result to disk cache atomically (write tmp, then rename).
pub fn write(cache_dir: &Path, id: &str, chart_updated: &str, data: &[u8]) -> anyhow::Result<()> {
    std::fs::create_dir_all(cache_dir)?;

    let bin_p = bin_path(cache_dir, id);
    let meta_p = meta_path(cache_dir, id);
    let bin_tmp = bin_p.with_extension("bin.tmp");
    let meta_tmp = meta_p.with_extension("meta.tmp");

    // Write bin
    std::fs::write(&bin_tmp, data)?;
    std::fs::rename(&bin_tmp, &bin_p)?;

    // Write meta
    let meta = CacheMeta {
        chart_updated: chart_updated.to_string(),
    };
    std::fs::write(&meta_tmp, serde_json::to_vec(&meta)?)?;
    std::fs::rename(&meta_tmp, &meta_p)?;

    Ok(())
}
