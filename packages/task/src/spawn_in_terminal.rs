use std::path::PathBuf;

use collections::HashMap;

use crate::{HideStrategy, RevealStrategy, RevealTarget, SaveStrategy, Shell};

/// Contains all information needed by Zed to spawn a new terminal tab for the given task.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct SpawnInTerminal {
    /// Id of the task to use when determining task tab affinity.
    pub id: crate::TaskId,
    /// Full unshortened form of `label` field.
    pub full_label: String,
    /// Human readable name of the terminal tab.
    pub label: String,
    /// Executable command to spawn.
    pub command: Option<String>,
    /// Arguments to the command, potentially unsubstituted,
    /// to let the shell that spawns the command to do the substitution, if needed.
    pub args: Vec<String>,
    /// A human-readable label, containing command and all of its arguments, joined and substituted.
    pub command_label: String,
    /// Current working directory to spawn the command into.
    pub cwd: Option<PathBuf>,
    /// Env overrides for the command, will be appended to the terminal's environment from the settings.
    pub env: HashMap<String, String>,
    /// Whether to use a new terminal tab or reuse the existing one to spawn the process.
    pub use_new_terminal: bool,
    /// Whether to allow multiple instances of the same task to be run, or rather wait for the existing ones to finish.
    pub allow_concurrent_runs: bool,
    /// What to do with the terminal pane and tab, after the command was started.
    pub reveal: RevealStrategy,
    /// Where to show tasks' terminal output.
    pub reveal_target: RevealTarget,
    /// What to do with the terminal pane and tab, after the command had finished.
    pub hide: HideStrategy,
    /// Which shell to use when spawning the task.
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
    pub fn to_proto(&self) -> proto::SpawnInTerminal {
        proto::SpawnInTerminal {
            label: self.label.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env: self
                .env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            cwd: self
                .cwd
                .clone()
                .map(|cwd| cwd.to_string_lossy().into_owned()),
        }
    }

    pub fn from_proto(proto: proto::SpawnInTerminal) -> Self {
        Self {
            label: proto.label.clone(),
            command: proto.command.clone(),
            args: proto.args.clone(),
            env: proto.env.into_iter().collect(),
            cwd: proto.cwd.map(PathBuf::from),
            ..Default::default()
        }
    }
}
