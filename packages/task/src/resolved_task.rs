use collections::HashSet;

use crate::{SpawnInTerminal, TaskId, TaskTemplate, VariableName};

/// A final form of the [`TaskTemplate`], that got resolved with a particular [`TaskContext`] and now is ready to spawn the actual task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTask {
    /// A way to distinguish tasks produced by the same template, but different contexts.
    /// NOTE: Resolved tasks may have the same labels, commands and do the same things,
    /// but still may have different ids if the context was different during the resolution.
    /// Since the template has `env` field, for a generic task that may be a bash command,
    /// so it's impossible to determine the id equality without more context in a generic case.
    pub id: TaskId,
    /// A template the task got resolved from.
    pub(crate) original_task: TaskTemplate,
    /// Full, unshortened label of the task after all resolutions are made.
    pub resolved_label: String,
    /// Variables that were substituted during the task template resolution.
    pub(crate) substituted_variables: HashSet<VariableName>,
    /// Further actions that need to take place after the resolved task is spawned,
    /// with all task variables resolved.
    pub resolved: SpawnInTerminal,
}

impl ResolvedTask {
    /// A task template before the resolution.
    pub fn original_task(&self) -> &TaskTemplate { &self.original_task }

    /// Variables that were substituted during the task template resolution.
    pub fn substituted_variables(&self) -> &HashSet<VariableName> { &self.substituted_variables }

    /// A human-readable label to display in the UI.
    pub fn display_label(&self) -> &str { self.resolved.label.as_str() }
}
