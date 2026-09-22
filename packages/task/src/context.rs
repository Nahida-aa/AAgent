use std::path::PathBuf;
use std::sync::Arc;

use collections::HashMap;

use crate::TaskVariables;

/// Keeps track of the file associated with a task and context of tasks execution (i.e. current file or current function).
/// Keeps all Zed-related state inside, used to produce a resolved task out of its template.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskContext {
    /// A path to a directory in which the task should be executed.
    pub cwd: Option<PathBuf>,
    /// Additional environment variables associated with a given task.
    pub task_variables: TaskVariables,
    /// Environment variables obtained when loading the project into Zed.
    /// This is the environment one would get when `cd`ing in a terminal
    /// into the project's root directory.
    pub project_env: HashMap<String, String>,
}

/// A shared reference to a [`TaskContext`], used to avoid cloning the context multiple times.
#[derive(Clone, Debug, Default)]
pub struct SharedTaskContext(Arc<TaskContext>);

impl std::ops::Deref for SharedTaskContext {
    type Target = TaskContext;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl From<TaskContext> for SharedTaskContext {
    fn from(context: TaskContext) -> Self { Self(Arc::new(context)) }
}
