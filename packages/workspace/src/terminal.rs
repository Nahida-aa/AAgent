//! TerminalProvider trait + OpenTerminal action — 对齐 zed `workspace`。
//!
//! - `TerminalProvider` — MultiWorkspace 通过 `Option<Box<dyn TerminalProvider>>`
//!   持有实现，由 App 层注入（terminal-view crate）。
//! - `OpenTerminal` — Action struct，带 working_directory + local 字段。
//!
//! Zed 对应: `crates/workspace/src/workspace.rs:327` (TerminalProvider) + :842 (OpenTerminal)。

use std::path::PathBuf;
use std::process::ExitStatus;

use aa_task::SpawnInTerminal;
use gpui::{Action, App, Task, Window};

/// 由终端面板实现的 trait — 抽象终端创建逻辑。
///
/// 消费方（MultiWorkspace / Workspace）只依赖此 trait，
/// 具体实现（terminal-view crate）通过 `set_terminal_provider` 注入。
///
/// 不加 Send + Sync — MultiWorkspace 在主线程，Box<dyn TerminalProvider> 也不跨线程。
/// 对齐 zed `workspace::TerminalProvider` (crates/workspace/src/workspace.rs:327)。
pub trait TerminalProvider {
    /// 在终端中 spawn 一个任务，返回 ExitStatus 的异步句柄。
    fn spawn(
        &self,
        task: SpawnInTerminal,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Result<ExitStatus, std::io::Error>>>;
}

/// Opens a new terminal with the specified working directory.
///
/// 对齐 zed `workspace::OpenTerminal` (crates/workspace/src/workspace.rs:842)。
#[derive(Debug, Default, Clone, serde::Deserialize, PartialEq, schemars::JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct OpenTerminal {
    pub working_directory: PathBuf,
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}

/// Opens a new terminal in the center.
#[derive(Default, PartialEq, Eq, Clone, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewCenterTerminal {
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}

/// Opens a new terminal.
#[derive(Default, PartialEq, Eq, Clone, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewTerminal {
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}
