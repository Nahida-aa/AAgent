//! Zed task template definition and resolution.
//!
//! A [`TaskTemplate`] is a declarative spec of a task, which may contain
//! [`VariableName`] placeholders. [`TaskTemplate::resolve_task`] substitutes
//! those placeholders against a [`TaskContext`] to produce a concrete
//! [`ResolvedTask`].

mod resolve;
mod strategies;
mod substitution;
mod template;
mod templates;

#[cfg(test)]
mod tests;

pub use strategies::{HideStrategy, RevealStrategy, SaveStrategy, TaskHook};
pub use substitution::{substitute_variables_in_map, substitute_variables_in_str};
pub use template::{DebugArgsRequest, TaskTemplate};
pub use templates::TaskTemplates;

// 内部共享
pub(crate) use substitution::{
    substitute_all_template_variables_in_str, to_hex_hash, truncate_variables,
};
