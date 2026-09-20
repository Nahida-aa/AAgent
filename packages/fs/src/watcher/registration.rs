//! 文件事件 watcher 注册 + 后端 — 对齐 Zed `crates/fs/src/fs_watcher.rs`。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use async_channel::{Receiver, Sender};
use notify::{Event, EventKind};

use super::diagnostics::{DiagnosticRecorder, OsWatcherKind, WatchDiagnosticEvent};

// ---------- PathEvent ----------

/// 路径事件类型 — Zed `PathEventKind`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathEventKind {
    Created,
    Changed,
    Removed,
    Rescan,
}

/// 一个路径事件。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathEvent {
    pub path: PathBuf,
    pub kind: Option<PathEventKind>,
}

// ---------- WatchBackend trait ----------

/// notify backend 抽象层 — Zed `WatchBackend`。
///
/// notify::Watcher trait 有 `Self: Sized`，不能直接做 dyn。Zed 用独立 trait 包装。
pub trait WatchBackend: Send {
    fn watch(&mut self, path: &Path, mode: notify::RecursiveMode) -> notify::Result<()>;
    fn unwatch(&mut self, path: &Path) -> notify::Result<()>;
}

impl<T: notify::Watcher + Send> WatchBackend for T {
    fn watch(&mut self, path: &Path, mode: notify::RecursiveMode) -> notify::Result<()> {
        notify::Watcher::watch(self, path, mode)
    }
    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        notify::Watcher::unwatch(self, path)
    }
}

/// 上层 watcher 接口 — 注册/注销路径。
pub trait Watcher: Send + Sync {
    fn add(&self, path: &Path) -> Result<()>;
    fn remove(&self, path: &Path) -> Result<()>;
}

// ---------- OsWatcher ----------

const POLL_INTERVAL: Duration = Duration::from_millis(2000);

/// 底层 OS watcher — Zed `OsWatcher`。
pub struct OsWatcher {
    kind: OsWatcherKind,
    backend: Mutex<Option<Box<dyn WatchBackend>>>,
    registrations: Arc<Mutex<HashMap<PathBuf, Box<dyn Fn(&Event) + Send + Sync>>>>,
    diagnostics: Arc<DiagnosticRecorder>,
    recursive: bool,
}

impl OsWatcher {
    pub fn new(kind: OsWatcherKind) -> Arc<Self> {
        let recursive = kind.is_recursive();
        Arc::new(Self {
            kind,
            backend: Mutex::new(None),
            registrations: Arc::new(Mutex::new(HashMap::new())),
            diagnostics: Arc::new(DiagnosticRecorder::default()),
            recursive,
        })
    }

    pub fn kind(&self) -> OsWatcherKind { self.kind }

    pub fn diagnostics(&self) -> &Arc<DiagnosticRecorder> { &self.diagnostics }

    fn ensure_backend(&self) -> Result<()> {
        let mut backend = self.backend.lock().unwrap();
        if backend.is_some() {
            return Ok(());
        }
        let event_sink = {
            let diagnostics = self.diagnostics.clone();
            let kind = self.kind;
            let registrations = self.registrations.clone(); // Mutex<...>: Clone
            move |res: notify::Result<Event>| {
                diagnostics.record(|| match &res {
                    Ok(event) => WatchDiagnosticEvent::new(
                        kind,
                        "event",
                        &event.paths,
                        format!("{:?}", event.kind),
                        matches!(event.kind, EventKind::Other),
                    ),
                    Err(error) => WatchDiagnosticEvent::new(
                        kind,
                        "error",
                        &error.paths,
                        format!("{error}"),
                        false,
                    ),
                });
                if let Ok(event) = res {
                    let regs = registrations.lock().unwrap();
                    for (registered_path, cb) in regs.iter() {
                        if event.paths.iter().any(|p| p.starts_with(registered_path)) {
                            cb(&event);
                        }
                    }
                }
            }
        };

        let watcher: Box<dyn WatchBackend> = match self.kind {
            OsWatcherKind::Native => {
                let config =
                    notify::Config::default().with_event_kinds(notify::EventKindMask::CORE);
                Box::new(<notify::RecommendedWatcher as notify::Watcher>::new(
                    event_sink, config,
                )?)
            }
            OsWatcherKind::Poll => {
                let config = notify::Config::default().with_poll_interval(POLL_INTERVAL);
                Box::new(notify::PollWatcher::new(event_sink, config)?)
            }
        };
        *backend = Some(watcher);
        Ok(())
    }

    pub fn add<F>(&self, path: PathBuf, cb: F) -> Result<()>
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.ensure_backend()?;
        let mode = if self.recursive {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };
        {
            let mut backend = self.backend.lock().unwrap();
            if let Some(b) = backend.as_mut() {
                b.watch(&path, mode)?;
            }
        }
        self.registrations
            .lock()
            .unwrap()
            .insert(path, Box::new(cb));
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<()> {
        let mut regs = self.registrations.lock().unwrap();
        if regs.remove(path).is_some() {
            if let Some(b) = self.backend.lock().unwrap().as_mut() {
                let _ = b.unwatch(path);
            }
        }
        Ok(())
    }
}

impl Drop for OsWatcher {
    fn drop(&mut self) { let _ = self.backend.lock().unwrap().take(); }
}

// ---------- FsWatcher ----------

/// 上层 watcher facade — Zed `FsWatcher`。
pub struct FsWatcher {
    native: Arc<OsWatcher>,
    poll: Arc<OsWatcher>,
    event_tx: Sender<PathEvent>,
    registrations: Mutex<HashMap<PathBuf, PathEventKind>>,
}

impl FsWatcher {
    pub fn new(native: Arc<OsWatcher>, poll: Arc<OsWatcher>, event_tx: Sender<PathEvent>) -> Self {
        Self {
            native,
            poll,
            event_tx,
            registrations: Mutex::new(HashMap::new()),
        }
    }
}

impl Watcher for FsWatcher {
    fn add(&self, path: &Path) -> Result<()> {
        let path = path.to_path_buf();
        if self.registrations.lock().unwrap().contains_key(&path) {
            return Ok(());
        }

        // 默认用 Poll（稳定），Native 可通过 env 切换
        let backend = if std::env::var("AA_WATCHER_MODE").as_deref() == Ok("native") {
            self.native.clone()
        } else {
            self.poll.clone()
        };

        let tx = self.event_tx.clone();
        let registered_path = path.clone();
        backend.add(path.clone(), move |event: &Event| {
            let kind = match event.kind {
                EventKind::Create(_) => Some(PathEventKind::Created),
                EventKind::Modify(_) => Some(PathEventKind::Changed),
                EventKind::Remove(_) => Some(PathEventKind::Removed),
                EventKind::Other => Some(PathEventKind::Rescan),
                _ => None,
            };
            for event_path in &event.paths {
                if event_path.starts_with(&registered_path) {
                    let _ = tx.try_send(PathEvent {
                        path: event_path.clone(),
                        kind,
                    });
                }
            }
        })?;

        self.registrations
            .lock()
            .unwrap()
            .insert(path, PathEventKind::Rescan);
        Ok(())
    }

    fn remove(&self, path: &Path) -> Result<()> {
        let mut regs = self.registrations.lock().unwrap();
        if regs.remove(path).is_some() {
            let _ = self.native.remove(path);
            let _ = self.poll.remove(path);
        }
        Ok(())
    }
}

// ---------- 便捷函数 ----------

pub fn create_default() -> (FsWatcher, Receiver<PathEvent>) {
    let native = OsWatcher::new(OsWatcherKind::Native);
    let poll = OsWatcher::new(OsWatcherKind::Poll);
    let (tx, rx) = async_channel::unbounded();
    (FsWatcher::new(native, poll, tx), rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_event_ordering() {
        let a = PathEvent {
            path: PathBuf::from("/a"),
            kind: Some(PathEventKind::Created),
        };
        let b = PathEvent {
            path: PathBuf::from("/b"),
            kind: Some(PathEventKind::Created),
        };
        assert!(a < b);
        assert_ne!(a, b);
    }

    #[test]
    fn test_os_watcher_add_remove() {
        let temp = tempfile::tempdir().unwrap();
        let w = OsWatcher::new(OsWatcherKind::Poll);
        let path = temp.path().to_path_buf();
        w.add(path.clone(), move |_event| {}).expect("add");
        w.remove(&path).expect("remove");
    }

    #[test]
    fn test_fs_watcher_smoke() {
        let temp = tempfile::tempdir().unwrap();
        let (watcher, rx) = create_default();
        watcher.add(temp.path()).expect("add temp dir");

        std::fs::write(temp.path().join("test.txt"), "hello").unwrap();
        std::thread::sleep(Duration::from_millis(2500));

        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        assert!(!events.is_empty(), "expected at least one file event");

        let _ = watcher.remove(temp.path());
    }
}
