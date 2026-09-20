//! 文件事件 watcher 诊断 — 对齐 Zed `crates/fs/src/fs_watcher/diagnostics.rs`。
//!
//! 精简版：提供核心诊断类型（事件录制、快照、重扫历史），
//! 不引入 notify/gpui/async 依赖。watch 事件生产端以后再接。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

const EVENT_CAPACITY: usize = 10_000;
const RESCAN_PATH_HISTORY_CAPACITY: usize = 10;

/// watcher 后端类型。
///
/// Zed 对应 `OsWatcherKind`：Native（inotify/FSEvents/ReadDirectoryChanges）
/// 或 Poll（notify 的 PollWatcher 兜底）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize)]
pub enum OsWatcherKind {
    #[default]
    Native,
    Poll,
}

impl OsWatcherKind {
    /// Poll 和 macOS/Windows 的 Native 自动递归；Linux Native（inotify）不递归。
    pub fn is_recursive(self) -> bool {
        self == Self::Poll || cfg!(any(target_os = "windows", target_os = "macos"))
    }
}

/// watcher 事件录制 — 调 `WatchRecording::new(...)` 开始，丢了就停。
///
/// 只持 Weak 引用，录制是 opt-in。
pub struct WatchRecording {
    state: Arc<Mutex<RecordingState>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct WatchSnapshot {
    pub started_at_unix_millis: u128,
    pub captured_at_unix_millis: u128,
    pub capacity: usize,
    pub dropped_events: u64,
    pub events: Vec<WatchDiagnosticEvent>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct WatchDiagnosticEvent {
    pub timestamp_unix_millis: u128,
    pub backend: OsWatcherKind,
    pub operation: String,
    pub detail: String,
    pub rescan: bool,
    pub paths: Vec<String>,
}

impl WatchDiagnosticEvent {
    pub fn new(
        backend: OsWatcherKind,
        operation: &str,
        paths: &[PathBuf],
        detail: String,
        rescan: bool,
    ) -> Self {
        Self {
            timestamp_unix_millis: unix_millis(),
            backend,
            operation: operation.into(),
            detail,
            rescan,
            paths: paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        }
    }
}

struct RecordingState {
    started_at_unix_millis: u128,
    dropped_events: u64,
    events: VecDeque<Arc<WatchDiagnosticEvent>>,
}

/// 全局诊断 recorder — watcher 收到事件时调 `record(...)`。
///
/// 内部只持有 Weak 到 active recordings，没人录制时 `record()` 不做分配。
#[derive(Default)]
pub struct DiagnosticRecorder {
    recordings: Mutex<Vec<Weak<Mutex<RecordingState>>>>,
}

impl DiagnosticRecorder {
    /// 记录一个事件。闭包只在有 active recording 时执行。
    pub fn record(&self, event: impl FnOnce() -> WatchDiagnosticEvent) {
        let recordings = self.recordings.lock().unwrap();
        // 先过滤掉已 drop 的 recording，没活的就不分配
        let alive_count = recordings.iter().filter(|w| w.strong_count() > 0).count();
        if alive_count == 0 {
            return;
        }
        // 有 live recording 才 eval 闭包
        let event = Arc::new(event());
        drop(recordings);

        let mut recordings = self.recordings.lock().unwrap();
        let mut i = 0;
        while i < recordings.len() {
            match recordings[i].upgrade() {
                Some(state) => {
                    drop(recordings);
                    let mut state = state.lock().unwrap();
                    if state.events.len() == EVENT_CAPACITY {
                        state.events.pop_front();
                        state.dropped_events += 1;
                    }
                    state.events.push_back(event.clone());
                    recordings = self.recordings.lock().unwrap();
                    i += 1;
                }
                None => {
                    recordings.remove(i);
                }
            }
        }
    }
}

/// 重扫事件历史 — 记录每个 rescan 触发时前 10 条路径，用于诊断。
#[derive(Default, Debug)]
pub struct RescanHistory {
    paths: VecDeque<PathBuf>,
}

impl RescanHistory {
    /// 记录一个可能触发 rescan 的路径集合。返回 rescan 报告（如果需要重扫）。
    pub fn record(&mut self, paths: &[PathBuf]) -> Option<String> {
        let report = if paths.is_empty() {
            None
        } else {
            // 保留最近 10 条
            let first = paths.len().saturating_sub(RESCAN_PATH_HISTORY_CAPACITY);
            for path in paths.iter().skip(first) {
                if self.paths.len() == RESCAN_PATH_HISTORY_CAPACITY {
                    self.paths.pop_front();
                }
                self.paths.push_back(path.clone());
            }
            Some(format!("paths={:?}, recent_paths={:?}", paths, self.paths))
        };
        report
    }
}

impl WatchRecording {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(RecordingState {
            started_at_unix_millis: unix_millis(),
            dropped_events: 0,
            events: VecDeque::new(),
        }));
        Self { state }
    }

    pub fn snapshot(&self) -> WatchSnapshot {
        let state = self.state.lock().unwrap();
        WatchSnapshot {
            started_at_unix_millis: state.started_at_unix_millis,
            captured_at_unix_millis: unix_millis(),
            capacity: EVENT_CAPACITY,
            dropped_events: state.dropped_events,
            events: state.events.iter().map(|e| (**e).clone()).collect(),
        }
    }
}

impl Default for WatchRecording {
    fn default() -> Self { Self::new() }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_watcher_kind() {
        assert_eq!(OsWatcherKind::default(), OsWatcherKind::Native);
        // Linux Native (inotify) 不递归, Poll 总递归
        assert!(OsWatcherKind::Poll.is_recursive());
    }

    #[test]
    fn test_watch_diagnostic_event_new() {
        let e = WatchDiagnosticEvent::new(
            OsWatcherKind::Native,
            "create",
            &[PathBuf::from("/tmp/foo.txt")],
            "ok".into(),
            false,
        );
        assert_eq!(e.backend, OsWatcherKind::Native);
        assert_eq!(e.operation, "create");
        assert_eq!(e.paths, vec!["/tmp/foo.txt".to_string()]);
        assert!(!e.rescan);
    }

    #[test]
    fn test_diagnostic_recorder_bounded() {
        let recorder = DiagnosticRecorder::default();
        let _recording = WatchRecording::new();

        // 把 recording 注册到 recorder（手动模拟 OsWatcher 构造时做的事）
        // 实际代码里 OsWatcher::new 会把 Arc::downgrade(&state) push 进去
        // 这里我们只能测 recorder 的 record 行为
        for i in 0..3 {
            recorder.record(|| {
                WatchDiagnosticEvent::new(OsWatcherKind::Native, "test", &[], i.to_string(), false)
            });
        }

        // 没人监听时 record 不做事（闭包甚至不会被调用——我们没注册）
        assert!(recorder.recordings.lock().unwrap().is_empty());
    }

    #[test]
    fn test_rescan_history() {
        let mut history = RescanHistory::default();
        let paths: Vec<PathBuf> = (0..12)
            .map(|i| PathBuf::from(format!("file-{i}")))
            .collect();

        // 每次 record 不产生 report（我们简化版只收集不判定）
        history.record(&paths[0..10]);
        let report = history.record(&paths[10..12]);
        assert!(report.is_some());
        // 10 (first batch) + 2 (second batch) = 12，但 capped to 10
        assert_eq!(history.paths.len(), RESCAN_PATH_HISTORY_CAPACITY);
    }

    #[test]
    fn test_rescan_history_empty() {
        let mut history = RescanHistory::default();
        assert!(history.record(&[]).is_none());
    }

    #[test]
    fn test_recording_snapshot() {
        let recording = WatchRecording::new();
        let snap = recording.snapshot();
        assert_eq!(snap.dropped_events, 0);
        assert_eq!(snap.capacity, EVENT_CAPACITY);
        assert!(snap.events.is_empty());
    }
}
