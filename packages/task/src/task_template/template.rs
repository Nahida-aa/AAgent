use collections::{HashMap, HashSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use util::serde::default_true;

use crate::serde_helpers::non_empty_string_vec;
use crate::{RevealTarget, Shell, VariableName};

use super::strategies::{HideStrategy, RevealStrategy, SaveStrategy, TaskHook};

/// A template definition of a Zed task to run.
/// May use the [`VariableName`] to get the corresponding substitutions into its fields.
///
/// Template itself is not ready to spawn a task, it needs to be resolved with a [`TaskContext`] first, that
/// contains all relevant Zed state in task variables.
/// A single template may produce different tasks (or none) for different contexts.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskTemplate {
    /// Human readable name of the task to display in the UI.
    pub label: String,
    /// Executable command to spawn.
    pub command: String,
    /// Arguments to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Env overrides for the command, will be appended to the terminal's environment from the settings.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Current working directory to spawn the command into, defaults to current project root.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Whether to use a new terminal tab or reuse the existing one to spawn the process.
    #[serde(default)]
    pub use_new_terminal: bool,
    /// Whether to allow multiple instances of the same task to be run, or rather wait for the existing ones to finish.
    #[serde(default)]
    pub allow_concurrent_runs: bool,
    /// What to do with the terminal pane and tab, after the command was started:
    /// * `always` — always show the task's pane, and focus the corresponding tab in it (default)
    /// * `no_focus` — always show the task's pane, add the task's tab in it, but don't focus it
    /// * `never` — do not alter focus, but still add/reuse the task's tab in its pane
    #[serde(default)]
    pub reveal: RevealStrategy,
    /// Where to place the task's terminal item after starting the task.
    /// * `dock` — in the terminal dock, "regular" terminal items' place (default).
    /// * `center` — in the central pane group, "main" editor area.
    #[serde(default)]
    pub reveal_target: RevealTarget,
    /// What to do with the terminal pane and tab, after the command had finished:
    /// * `never` — do nothing when the command finishes (default)
    /// * `always` — always hide the terminal tab, hide the pane also if it was the last tab in it
    /// * `on_success` — hide the terminal tab on task success only, otherwise behaves similar to `always`.
    #[serde(default)]
    pub hide: HideStrategy,
    /// Represents the tags which this template attaches to.
    /// Adding this removes this task from other UI and gives you ability to run it by tag.
    #[serde(default, deserialize_with = "non_empty_string_vec")]
    #[schemars(length(min = 1))]
    pub tags: Vec<String>,
    /// Which shell to use when spawning the task.
    #[serde(default)]
    pub shell: Shell,
    /// Whether to show the task line in the task output.
    #[serde(default = "default_true")]
    pub show_summary: bool,
    /// Whether to show the command line in the task output.
    #[serde(default = "default_true")]
    pub show_command: bool,
    /// Which edited buffers to save before running the task.
    #[serde(default)]
    pub save: SaveStrategy,
    /// Hooks that this task runs when emitted.
    #[serde(default)]
    pub hooks: HashSet<TaskHook>,
}

/// Use to represent debug request type
#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub enum DebugArgsRequest {
    /// launch (program, cwd) are stored in TaskTemplate as (command, cwd)
    Launch,
    /// Attach
    Attach(crate::AttachRequest),
}
