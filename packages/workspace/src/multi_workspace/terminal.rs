//! TerminalProvider trait — 对齐 zed `workspace::TerminalProvider`。
//!
//! MultiWorkspace 通过 `Option<Box<dyn TerminalProvider>>` 持有实现，
//! 由 App 层注入（terminal-view crate 实现此 trait）。
//! 这样 workspace crate 不依赖 terminal-view crate（单向依赖 OK）。
//!
//! Zed 定义在 `crates/workspace/src/workspace.rs:327`。

use std::process::ExitStatus;

use gpui::{App, Task, Window};

/// 启动终端时需要的参数。
///
/// Zed 用 `task::SpawnInTerminal`（对齐 `crates/task/src/task.rs:42`），
/// 包含 command、working_directory、env、shell 等完整信息。
/// AAgent 现阶段只暴露核心字段，后续对齐 Zed 再补。
#[derive(Debug, Clone, Default)]
pub struct SpawnInTerminal {
    /// Shell 命令。None = 启动交互式 shell。
    pub command: Option<String>,
    /// 工作目录。
    pub working_directory: std::path::PathBuf,
}

/// 由终端面板实现的 trait — 抽象终端创建逻辑。
///
/// 消费方（MultiWorkspace / Workspace）只依赖此 trait，
/// 具体实现（terminal-view crate）通过 `set_terminal_provider` 注入。
pub trait TerminalProvider: Send + Sync {
    /// 在终端中 spawn 一个任务，返回 ExitStatus 的异步句柄。
    fn spawn(
        &self,
        task: SpawnInTerminal,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Result<ExitStatus, std::io::Error>>>;
}
