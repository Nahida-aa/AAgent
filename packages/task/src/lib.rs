//! AA Task — 任务定义与执行抽象。
//!
//! 对齐 zed `crates/task/src/task.rs`：
//! - `SpawnInTerminal` — 启动终端任务所需的完整参数
//! - 后续会有 TaskId、ResolvedTask、TaskContext 等

use std::collections::HashMap;
use std::path::PathBuf;

/// 任务唯一标识。
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

/// 启动终端时需要的完整参数。
///
/// 对齐 zed `task::SpawnInTerminal` (crates/task/src/task.rs:42)。
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct SpawnInTerminal {
    /// Id of the task to use when determining task tab affinity.
    pub id: TaskId,
    /// Human readable name of the terminal tab.
    pub label: String,
    /// Executable command to spawn. None = 启动交互式 shell。
    pub command: Option<String>,
    /// Arguments to the command.
    pub args: Vec<String>,
    /// Current working directory to spawn the command into.
    pub cwd: Option<PathBuf>,
    /// Env overrides for the command.
    pub env: HashMap<String, String>,
    /// Whether to use a new terminal tab or reuse the existing one.
    pub use_new_terminal: bool,
    /// Whether to allow multiple instances of the same task.
    pub allow_concurrent_runs: bool,
}
