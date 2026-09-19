//! 文件事件 watcher — 精简版。
//!
//! 对齐 Zed `crates/fs/src/fs_watcher.rs`，但只提供**数据类型 + 诊断设施**，
//! 不引入 notify/gpui/async_channel 等重依赖。真正的 watcher backend 以后接入。

pub mod diagnostics;

pub use diagnostics::{
    DiagnosticRecorder, OsWatcherKind, RescanHistory, WatchDiagnosticEvent, WatchRecording,
    WatchSnapshot,
};
