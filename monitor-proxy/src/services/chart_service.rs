use crate::{
    error::{AppErrorExt, Result},
    utils::{load_chart, FileCache, FileCacheResult, ResourceLoader},
    AppState,
};
use bincode::Options;
use bytes::Bytes;
use futures::Stream;
use log::info;
use monitor_common::core::{AnimFloat, Chart, ChartInfo, JudgeLine, Keyframe, Note, NoteKind};
use serde_json::json;
use std::{
    future::Future,
    path::{Component, Path, PathBuf},
    pin::Pin,
};
use tempfile::{tempdir, tempfile};
use tokio::{
    fs::File,
    io::{self, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

pub struct ChartService {}

impl ChartService {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle_chart_request(
        &self,
        state: &AppState,
        id: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, io::Error>>> {
        let info_json;
        let chart_updated = if id == "test" {
            info_json = json!(null);
            ""
        } else {
            info_json = state
                .http_client
                .get(format!("{}/chart/{}", state.config.api_base, id))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .bad_request("failed to fetch chart info")?
                .json::<serde_json::Value>()
                .await
                .internal_server_error("failed to parse chart info")?;
            info_json["chartUpdated"].as_str().unwrap_or("")
        };

        let f = match FileCache::acquire(&state.config.cache_dir, id, chart_updated).await? {
            FileCacheResult::Hit(f) => {
                info!("Chart {id} served from cache");
                f
            }
            FileCacheResult::Miss(mut f) => {
                let (info, chart) = if id == "test" {
                    // generate test chart
                    Self::generate_test_chart()
                } else {
                    info!("Chart {id} cache miss, downloading...");
                    let url = info_json["file"]
                        .as_str()
                        .internal_server_error("no file url found in chart metadata")?;
                    Self::download_chart(state, url).await?
                };
                Self::serialize_chart(&info, &chart, &mut f).await?;
                f.update(chart_updated).await?
            }
        };
        Ok(ReaderStream::new(f))
    }

    fn generate_test_chart() -> (ChartInfo, Chart) {
        let mut line = JudgeLine::default();
        const HEIGHT_PER_SEC: f32 = 1.0;

        line.height = AnimFloat::new(vec![
            Keyframe::new(0.0, 0.0, 2),
            Keyframe::new(100.0, 100.0 * HEIGHT_PER_SEC, 0),
        ]);

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

        (info, chart)
    }

    async fn download_chart(state: &AppState, url: &str) -> Result<(ChartInfo, Chart)> {
        let mut tmp_file =
            File::from_std(tempfile().internal_server_error("failed to create temp file")?);
        let tmp_dir = tempdir().internal_server_error("failed to create temp directory")?;

        info!("Downloading chart from {url}");
        let mut byte_stream = state
            .http_client
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .internal_server_error("failed to download zip")?
            .bytes_stream();
        while let Some(chunk) = byte_stream.next().await {
            let data = chunk.internal_server_error("failed to download zip")?;
            tmp_file
                .write_all(&data)
                .await
                .internal_server_error("failed to write to tmp file")?;
        }

        tokio::task::spawn_blocking({
            let tmp_file = tmp_file.into_std().await;
            let out_path = tmp_dir.path().to_path_buf();
            move || -> Result<()> {
                let mut archive = zip::ZipArchive::new(tmp_file)?;
                archive.extract(out_path)?;
                Ok(())
            }
        })
        .await??;

        load_chart(&mut DirectoryLoader::new(tmp_dir.path()))
            .await
            .internal_server_error("failed to load chart")
    }

    async fn serialize_chart<W>(info: &ChartInfo, chart: &Chart, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin + Send,
    {
        tokio::task::block_in_place(|| -> Result<(), io::Error> {
            let sync_writer = tokio_util::io::SyncIoBridge::new(writer);
            let mut buf_writer = std::io::BufWriter::with_capacity(64 * 1024, sync_writer);

            bincode::options()
                .with_varint_encoding()
                .serialize_into(&mut buf_writer, &(info, chart))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            buf_writer.into_inner().map_err(|e| e.into_error())?;
            Ok(())
        })
        .internal_server_error("failed to serialize chart")
    }
}

struct DirectoryLoader {
    directory: PathBuf,
}

impl DirectoryLoader {
    pub fn new<P>(dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            directory: dir.into(),
        }
    }
}

impl ResourceLoader for DirectoryLoader {
    fn load_file<'a>(
        &'a mut self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>> {
        let mut safe_path = PathBuf::new();
        for component in Path::new(path).components() {
            match component {
                Component::Normal(name) => safe_path.push(name),
                Component::ParentDir => {
                    safe_path.pop();
                }
                _ => {}
            }
        }
        let safe_path = self.directory.join(safe_path);
        Box::pin(async move {
            let mut file = File::open(safe_path).await?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).await?;
            Ok(buf)
        })
    }
}
