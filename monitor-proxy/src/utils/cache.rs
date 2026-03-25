use anyhow::{Context as _Context, Result};
use fs4::tokio::AsyncFileExt;
use pin_project::pin_project;
use serde::Serialize;
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};
use tempfile::TempPath;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{self, AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, SeekFrom},
};

pub enum FileCacheResult<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    Hit(FileCacheHitGuard<M>),
    Miss(FileCacheMissGuard<M>),
}

#[pin_project]
pub struct FileCacheHitGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    #[pin]
    bin_file: File,
    _marker: PhantomData<M>,
}

#[pin_project]
pub struct FileCacheMissGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    meta_file: File,
    bin_path: PathBuf,
    tmp_path: TempPath,
    #[pin]
    tmp_file: File,
    _marker: PhantomData<M>,
}

pub struct FileCache<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    _marker: PhantomData<M>,
}

impl<M> FileCache<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    fn meta_path(cache_dir: &Path, id: &str) -> PathBuf {
        cache_dir.join(format!("{}.meta", id))
    }

    fn bin_path(cache_dir: &Path, id: &str) -> PathBuf {
        cache_dir.join(format!("{}.bin", id))
    }

    pub async fn acquire(cache_dir: &Path, id: &str, meta: &M) -> Result<FileCacheResult<M>> {
        // Ensure cache directory exists
        std::fs::create_dir_all(cache_dir).context("failed to create cache dir")?;
        let meta_p = Self::meta_path(cache_dir, id);
        let bin_p = Self::bin_path(cache_dir, id);

        // Blocking: open + flock + read meta
        let (meta_file, cached_meta) = tokio::task::spawn({
            let meta_p = meta_p.clone();
            async move {
                let mut f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&meta_p)
                    .await?;
                f.lock_exclusive()?;
                let mut cached_meta = String::new();
                f.read_to_string(&mut cached_meta).await?;
                Result::<_>::Ok((f, cached_meta))
            }
        })
        .await??;

        let meta = serde_json::to_string(&meta).context("failed to serialize meta")?;
        Ok(if cached_meta == meta {
            drop(meta_file);
            FileCacheResult::Hit(FileCacheHitGuard::new(bin_p).await?)
        } else {
            FileCacheResult::Miss(FileCacheMissGuard::new(meta_file, bin_p).await?)
        })
    }
}

impl<M> FileCacheHitGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    pub(self) async fn new(bin_path: PathBuf) -> Result<Self> {
        let bin_file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(bin_path)
            .await?;
        Ok(Self {
            bin_file,
            _marker: PhantomData,
        })
    }

    pub(self) async fn from_miss(miss_guard: FileCacheMissGuard<M>) -> Result<Self, io::Error> {
        miss_guard.meta_file.unlock_async().await?;
        Ok(Self {
            bin_file: File::open(miss_guard.bin_path).await?,
            _marker: PhantomData,
        })
    }
}

impl<M> AsyncRead for FileCacheHitGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().bin_file.poll_read(cx, buf)
    }
}

impl<M> FileCacheMissGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    pub(self) async fn new(meta_file: File, bin_path: PathBuf) -> Result<Self> {
        let (tmp_file, tmp_path) = tempfile::Builder::new()
            .prefix(".tmp_")
            .tempfile_in(bin_path.parent().unwrap_or_else(|| Path::new(".")))?
            .into_parts();
        Ok(Self {
            meta_file,
            bin_path,
            tmp_path,
            tmp_file: File::from_std(tmp_file),
            _marker: PhantomData,
        })
    }

    pub async fn update(mut self, meta: &M) -> Result<FileCacheHitGuard<M>, io::Error> {
        let data = serde_json::to_vec(meta)?;
        fs::rename(&self.tmp_path, &self.bin_path).await?;
        self.meta_file.set_len(0).await?;
        self.meta_file.seek(SeekFrom::Start(0)).await?;
        self.meta_file.write_all(&data).await?;
        FileCacheHitGuard::from_miss(self).await
    }
}

impl<M> AsyncWrite for FileCacheMissGuard<M>
where
    M: Serialize + Send + Sync + ?Sized,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.project().tmp_file.poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.project().tmp_file.poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.project().tmp_file.poll_shutdown(cx)
    }
    fn is_write_vectored(&self) -> bool {
        self.tmp_file.is_write_vectored()
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        self.project().tmp_file.poll_write_vectored(cx, bufs)
    }
}
