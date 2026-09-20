//! RealFs — 真实 OS 文件系统实现。

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::Stream;

use crate::{Fs, Metadata, PathEvent, Watcher};

/// 真实 OS 文件系统实现——生产环境用。
pub struct RealFs;

impl RealFs {
    pub fn new() -> Arc<dyn Fs> { Arc::new(Self) }
}

impl Default for RealFs {
    fn default() -> Self { Self }
}

#[async_trait::async_trait]
impl Fs for RealFs {
    async fn is_file(&self, path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }

    async fn is_dir(&self, path: &Path) -> bool {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }

    async fn path_exists(&self, path: &Path) -> bool { path.exists() }

    async fn load_bytes(&self, path: &Path) -> Result<Vec<u8>> { Ok(std::fs::read(path)?) }

    async fn create_dir(&self, path: &Path) -> Result<()> { Ok(std::fs::create_dir_all(path)?) }

    async fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(std::fs::write(path, content)?)
    }

    async fn remove_file(&self, path: &Path) -> Result<()> { Ok(std::fs::remove_file(path)?) }

    async fn remove_dir(&self, path: &Path, recursive: bool) -> Result<()> {
        if recursive {
            Ok(std::fs::remove_dir_all(path)?)
        } else {
            Ok(std::fs::remove_dir(path)?)
        }
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> { Ok(std::fs::rename(from, to)?) }

    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        Ok(std::fs::canonicalize(path)?)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)? {
            out.push(entry?.path());
        }
        Ok(out)
    }

    async fn metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(Metadata {
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            len: meta.len(),
        }))
    }

    async fn watch(
        &self,
        _path: &Path,
        _latency: Duration,
    ) -> (
        Pin<Box<dyn Send + Stream<Item = Vec<PathEvent>>>>,
        Arc<dyn Watcher>,
    ) {
        // 返回一个永不产生事件的 stream + noop watcher
        let stream: Pin<Box<dyn Send + Stream<Item = Vec<PathEvent>>>> =
            Box::pin(futures::stream::empty());
        let watcher = Arc::new(NoopWatcher);
        (stream, watcher)
    }
}

/// 空实现的 Watcher — 测试/精简模式用。
pub struct NoopWatcher;

impl Watcher for NoopWatcher {
    fn add(&self, _path: &Path) -> Result<()> { Ok(()) }
    fn remove(&self, _path: &Path) -> Result<()> { Ok(()) }
}
