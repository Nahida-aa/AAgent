//! 文件事件 watcher — 带 notify 后端。
//!
//! 对齐 Zed `crates/fs/src/fs_watcher.rs`，精简版：
//! - diagnostics: 事件录制 / 诊断（零后端依赖）
//! - registration: OsWatcher + FsWatcher（notify backend）

pub mod diagnostics;
pub mod registration;

pub use diagnostics::{
    DiagnosticRecorder, OsWatcherKind, RescanHistory, WatchDiagnosticEvent, WatchRecording,
    WatchSnapshot,
};
pub use registration::{FsWatcher, OsWatcher, PathEvent, PathEventKind, Watcher, create_default};
