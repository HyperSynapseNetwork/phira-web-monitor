use anyhow::Result;
use fs2::FileExt;
use monitor_common::core::{Chart, ChartInfo};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize, serde::Serialize)]
struct CacheMeta {
    chart_updated: String,
}

fn meta_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join(format!("{}.meta", id))
}

fn bin_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join(format!("{}.bin", id))
}

pub enum CacheResult {
    Hit(PathBuf),
    Miss(CacheGuard),
}

pub struct CacheGuard {
    meta_file: File,
    bin_path: PathBuf,
}

pub async fn acquire(cache_dir: &Path, id: &str, chart_updated: &str) -> Result<CacheResult> {
    let meta_p = meta_path(cache_dir, id);
    let bin_p = bin_path(cache_dir, id);
    let chart_updated = chart_updated.to_string();

    // Ensure cache directory exists
    std::fs::create_dir_all(cache_dir)?;

    // Blocking: open + flock + read meta
    let (meta_file, cached_meta) = tokio::task::spawn_blocking({
        let meta_p = meta_p.clone();
        move || -> anyhow::Result<(File, Option<CacheMeta>)> {
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&meta_p)?;
            f.lock_exclusive()?;

            let meta = serde_json::from_reader::<_, CacheMeta>(&f).ok();
            Ok((f, meta))
        }
    })
    .await??;

    // Check if cache is still valid
    if let Some(ref meta) = cached_meta {
        if meta.chart_updated == chart_updated {
            // Hit — release the lock, return the path for streaming
            drop(meta_file);
            return Ok(CacheResult::Hit(bin_p));
        }
    }

    // Miss — keep the lock; caller will produce data and call guard.write()
    Ok(CacheResult::Miss(CacheGuard {
        meta_file,
        bin_path: bin_p,
    }))
}

impl CacheGuard {
    pub fn write(mut self, chart_updated: &str, value: &(ChartInfo, Chart)) -> Result<PathBuf> {
        // Serialize directly to disk via BufWriter (no Vec<u8>)
        let bin_tmp = self.bin_path.with_extension("bin.tmp");
        {
            let file = File::create(&bin_tmp)?;
            let writer = BufWriter::new(file);
            use bincode::Options;
            bincode::options()
                .with_varint_encoding()
                .serialize_into(writer, value)?;
        }
        std::fs::rename(&bin_tmp, &self.bin_path)?;

        // Rewrite .meta via the locked file handle
        self.meta_file.set_len(0)?;
        self.meta_file.seek(SeekFrom::Start(0))?;
        let meta = CacheMeta {
            chart_updated: chart_updated.to_string(),
        };
        self.meta_file.write_all(&serde_json::to_vec(&meta)?)?;

        // Lock released when self.meta_file is dropped
        Ok(self.bin_path)
    }
}
