use async_channel::Sender;
use std::process::ExitStatus;
use task::SpawnInTerminal;

use super::task::{TaskState, TaskStatus};

pub struct TerminalMode(TerminalModeKind);

pub(super) enum TerminalModeKind {
    Interactive,
    InteractiveWithCompletion(Sender<Option<ExitStatus>>),
    Task {
        state: TaskState,
        completion_tx: Sender<Option<ExitStatus>>,
    },
}

impl TerminalMode {
    /// Creates a terminal for an interactive shell.
    pub fn interactive() -> Self { Self(TerminalModeKind::Interactive) }

    /// Creates an interactive terminal that reports when its shell exits.
    pub fn interactive_with_completion(completion_tx: Sender<Option<ExitStatus>>) -> Self {
        Self(TerminalModeKind::InteractiveWithCompletion(completion_tx))
    }

    /// Creates a running task terminal with an internally paired completion channel.
    pub fn task(spawned_task: SpawnInTerminal) -> Self {
        let (completion_tx, completion_rx) = async_channel::bounded(1);
        Self(TerminalModeKind::Task {
            state: TaskState {
                status: TaskStatus::Running,
                completion_rx,
                spawned_task,
            },
            completion_tx,
        })
    }
}
