//! Baseline interface of Tasks in Zed: all tasks in Zed are intended to use those for implementing their own logic.

mod adapter_schema;
mod context;
mod debug_format;
mod env_replacer;
mod resolved_task;
mod runnable_tag;
mod serde_helpers;
mod shell_proto;
mod spawn_in_terminal;
mod task_id;
mod task_template;
mod variable;
mod variables;
mod vscode_debug_format;
mod vscode_format;

pub mod static_source;

pub use aagent_actions::RevealTarget;
pub use adapter_schema::{AdapterSchema, AdapterSchemas};
pub use context::{SharedTaskContext, TaskContext};
pub use debug_format::{
    AttachRequest, BuildTaskDefinition, DebugRequest, DebugScenario, DebugTaskFile, LaunchRequest,
    Request, TcpArgumentsTemplate, ZedDebugConfig,
};
pub use resolved_task::ResolvedTask;
pub use runnable_tag::RunnableTag;
pub use shell_proto::{shell_from_proto, shell_to_proto};
pub use spawn_in_terminal::SpawnInTerminal;
pub use task_id::TaskId;
pub use task_template::{
    DebugArgsRequest, HideStrategy, RevealStrategy, SaveStrategy, TaskHook, TaskTemplate,
    TaskTemplates, substitute_variables_in_map, substitute_variables_in_str,
};
pub use util::shell::{Shell, ShellKind};
pub use util::shell_builder::ShellBuilder;
pub use variable::{VariableName, ZED_VARIABLE_NAME_PREFIX};
pub use variables::TaskVariables;
pub use vscode_debug_format::VsCodeDebugTaskFile;
pub use vscode_format::VsCodeTaskFile;

pub(crate) use env_replacer::{
    EnvVariableReplacer, VsCodeCommand, VsCodeEnvVariable, ZedEnvVariable,
};
pub(crate) use variable::ZED_CUSTOM_VARIABLE_NAME_PREFIX;
