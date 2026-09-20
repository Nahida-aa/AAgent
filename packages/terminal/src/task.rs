use async_channel::{Receiver, Sender};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use task::SpawnInTerminal;

/// Configures whether a terminal runs an interactive shell or a tracked task.
///
/// Task modes must be created with [`TerminalMode::task`] so their completion
/// sender and receiver remain paired.
pub struct TerminalMode(TerminalModeKind);

enum TerminalModeKind {
    Interactive,
    InteractiveWithCompletion(Sender<Option<ExitStatus>>),
    Task {
        state: TaskState,
        completion_tx: Sender<Option<ExitStatus>>,
    },
}

impl TerminalMode {
    /// Creates a terminal for an interactive shell.
    pub fn interactive() -> Self {
        Self(TerminalModeKind::Interactive)
    }

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

/// Runtime state for a task-backed terminal.
#[derive(Debug)]
pub struct TaskState {
    pub status: TaskStatus,
    /// Kept private so it can only be paired by [`TerminalMode::task`] with the
    /// sender that reports this task's completion.
    completion_rx: Receiver<Option<ExitStatus>>,
    pub spawned_task: SpawnInTerminal,
}

/// A status of the current terminal tab's task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// The task had been started, but got cancelled or somehow otherwise it did not
    /// report its exit code before the terminal event loop was shut down.
    Unknown,
    /// The task is started and running currently.
    Running,
    /// After the start, the task stopped running and reported its error code back.
    Completed { success: bool },
}

const TASK_DELIMITER: &str = "⏵ ";
fn task_summary(task: &TaskState, exit_status: Option<ExitStatus>) -> (bool, String, String) {
    let escaped_full_label = task
        .spawned_task
        .full_label
        .replace("\r\n", "\r")
        .replace('\n', "\r");
    let task_label = |suffix: &str| format!("{TASK_DELIMITER}Task `{escaped_full_label}` {suffix}");
    let (success, task_line) = match exit_status {
        Some(status) => {
            let code = status.code();
            #[cfg(unix)]
            let signal = status.signal();
            #[cfg(not(unix))]
            let signal: Option<i32> = None;

            match (code, signal) {
                (Some(0), _) => (true, task_label("finished successfully")),
                (Some(code), _) => (
                    false,
                    task_label(&format!("finished with exit code: {code}")),
                ),
                (None, Some(signal)) => (
                    false,
                    task_label(&format!("terminated by signal: {signal}")),
                ),
                (None, None) => (false, task_label("finished")),
            }
        }
        None => (false, task_label("finished")),
    };
    let escaped_command_label = task
        .spawned_task
        .command_label
        .replace("\r\n", "\r")
        .replace('\n', "\r");
    let command_line = format!("{TASK_DELIMITER}Command: {escaped_command_label}");
    (success, task_line, command_line)
}
