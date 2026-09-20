//! SpawnInTerminal — 启动终端任务所需的完整参数。
//!
//! 对齐 Zed `crates/task/src/task.rs::SpawnInTerminal`（行 42）。
//!
//! Shell quoting / command 构造逻辑委托 `util::shell_builder::ShellBuilder`。

use aagent_actions::RevealTarget;
use collections::HashMap;
use std::path::PathBuf;

use util::shell::Shell;

use crate::{HideStrategy, RevealStrategy, SaveStrategy};

/// 任务唯一标识。
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

/// 启动终端时需要的完整参数。
///
/// 对齐 Zed `task::SpawnInTerminal`，精简版：目前没有 reveal/hide 策略、
/// shell 等完整任务配置字段——那些等 TaskTemplate resolve 时再加。
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct SpawnInTerminal {
    /// Id of the task to use when determining task tab affinity.
    pub id: TaskId,
    /// Full unshortened form of `label` field.
    pub full_label: String,
    /// Human readable name of the terminal tab.
    pub label: String,
    /// Executable command to spawn. None = 启动交互式 shell。
    pub command: Option<String>,
    /// Arguments to the command.
    pub args: Vec<String>,
    /// A human-readable label, containing command and all of its arguments, joined and substituted.
    pub command_label: String,
    /// Current working directory to spawn the command into.
    pub cwd: Option<PathBuf>,
    /// Env overrides for the command.
    pub env: HashMap<String, String>,
    /// Whether to use a new terminal tab or reuse the existing one.
    pub use_new_terminal: bool,
    /// Whether to allow multiple instances of the same task.
    pub allow_concurrent_runs: bool,
    /// What to do with the terminal pane and tab, after the command was started.
    pub reveal: RevealStrategy,
    /// Where to show tasks' terminal output.
    pub reveal_target: RevealTarget,
    /// What to do with the terminal pane and tab, after the command had finished.
    pub hide: HideStrategy,
    /// Shell configuration。后续接入 ShellBuilder 时用。
    pub shell: Shell,
    /// Whether to show the task summary line in the task output (success/failure).
    pub show_summary: bool,
    /// Whether to show the command line in the task output.
    pub show_command: bool,
    /// Whether to show the rerun button in the terminal tab.
    pub show_rerun: bool,
    /// Which edited buffers to save before running the task.
    pub save: SaveStrategy,
}

impl SpawnInTerminal {
    /// 从 command + args 拼出人类可读的完整命令行标签。
    ///
    /// 对齐 Zed `SpawnInTerminal::command_label`（task_template.rs:281）。
    pub fn build_command_label(command: Option<&str>, args: &[String]) -> String {
        let Some(cmd) = command else {
            return String::new();
        };
        args.iter().fold(cmd.to_string(), |mut label, arg| {
            label.push(' ');
            label.push_str(arg);
            label
        })
    }

    /// 返回最终要执行的 command（如果有）+ args。
    /// 等价于直接读 `self.command` 和 `self.args`，但语义更明确。
    pub fn parts(&self) -> (Option<String>, Vec<String>) {
        (self.command.clone(), self.args.clone())
    }
}
