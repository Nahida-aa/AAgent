//! Task template 策略枚举。
//!
//! 对齐 Zed `task::RevealStrategy` / `HideStrategy` / `SaveStrategy` / `TaskHook`
//! (crates/task/src/task_template.rs:95-137)。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What to do with the terminal pane and tab, after the command was started.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevealStrategy {
    /// Always show the task's pane, and focus the corresponding tab in it.
    #[default]
    Always,
    /// Always show the task's pane, add the task's tab in it, but don't focus it.
    NoFocus,
    /// Do not alter focus, but still add/reuse the task's tab in its pane.
    Never,
}

/// What to do with the terminal pane and tab, after the command has finished.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HideStrategy {
    /// Do nothing when the command finishes.
    #[default]
    Never,
    /// Always hide the terminal tab, hide the pane also if it was the last tab in it.
    Always,
    /// Hide the terminal tab on task success only, otherwise behaves similar to `Always`.
    OnSuccess,
}

/// Which edited buffers to save before running a task.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SaveStrategy {
    /// Save all edited buffers.
    All,
    /// Save the current buffer.
    Current,
    #[default]
    /// Don't save any buffers.
    None,
}

/// Task hook — 特殊 task actions，不是普通 shell 命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskHook {
    /// Create a new git worktree before running the task.
    #[serde(alias = "create_git_worktree")]
    CreateWorktree,
}
