use anyhow::Context as _;
use collections::{HashMap, HashSet};
use std::path::PathBuf;
use util::ResultExt as _;

use crate::{
    ResolvedTask, SpawnInTerminal, TaskContext, TaskId, VariableName, ZED_VARIABLE_NAME_PREFIX,
};

use super::substitution::{
    substitute_all_template_variables_in_map, substitute_all_template_variables_in_str,
    substitute_all_template_variables_in_vec, to_hex_hash, truncate_variables,
};
use super::template::TaskTemplate;

impl TaskTemplate {
    /// Replaces all `VariableName` task variables in the task template string fields.
    ///
    /// Every [`ResolvedTask`] gets a [`TaskId`], based on the `id_base` (to avoid collision with various task sources),
    /// and hashes of its template and [`TaskContext`], see [`ResolvedTask`] fields' documentation for more details.
    pub fn resolve_task(&self, id_base: &str, cx: &TaskContext) -> Option<ResolvedTask> {
        if self.label.trim().is_empty() || self.command.trim().is_empty() {
            return None;
        }

        let mut variable_names = HashMap::default();
        let mut substituted_variables = HashSet::default();
        let task_variables = cx
            .task_variables
            .0
            .iter()
            .map(|(key, value)| {
                let key_string = key.to_string();
                if !variable_names.contains_key(&key_string) {
                    variable_names.insert(key_string.clone(), key.clone());
                }
                (key_string, value.as_str())
            })
            .collect::<HashMap<_, _>>();
        let truncated_variables = truncate_variables(&task_variables);
        let cwd = match self.cwd.as_deref() {
            Some(cwd) => {
                let substituted_cwd = substitute_all_template_variables_in_str(
                    cwd,
                    &task_variables,
                    &variable_names,
                    &mut substituted_variables,
                )?;
                Some(PathBuf::from(substituted_cwd))
            }
            None => None,
        }
        .or(cx.cwd.clone());
        let full_label = substitute_all_template_variables_in_str(
            &self.label,
            &task_variables,
            &variable_names,
            &mut substituted_variables,
        )?;

        // Arbitrarily picked threshold below which we don't truncate any variables.
        const TRUNCATION_THRESHOLD: usize = 64;

        let human_readable_label = if full_label.len() > TRUNCATION_THRESHOLD {
            substitute_all_template_variables_in_str(
                &self.label,
                &truncated_variables,
                &variable_names,
                &mut substituted_variables,
            )?
        } else {
            #[allow(
                clippy::redundant_clone,
                reason = "We want to clone the full_label to avoid borrowing it in the fold closure"
            )]
            full_label.clone()
        }
        .lines()
        .fold(String::new(), |mut string, line| {
            if string.is_empty() {
                string.push_str(line);
            } else {
                string.push_str("\\n");
                string.push_str(line);
            }
            string
        });

        let command = substitute_all_template_variables_in_str(
            &self.command,
            &task_variables,
            &variable_names,
            &mut substituted_variables,
        )?;
        let args_with_substitutions = substitute_all_template_variables_in_vec(
            &self.args,
            &task_variables,
            &variable_names,
            &mut substituted_variables,
        )?;

        let task_hash = to_hex_hash(self)
            .context("hashing task template")
            .log_err()?;
        let variables_hash = to_hex_hash(&task_variables)
            .context("hashing task variables")
            .log_err()?;
        let id = TaskId(format!("{id_base}_{task_hash}_{variables_hash}"));

        let env = {
            // Start with the project environment as the base.
            let mut env = cx.project_env.clone();

            // Extend that environment with what's defined in the TaskTemplate
            env.extend(self.env.clone());

            // Then we replace all task variables that could be set in environment variables
            let mut env = substitute_all_template_variables_in_map(
                &env,
                &task_variables,
                &variable_names,
                &mut substituted_variables,
            )?;

            // Last step: set the task variables as environment variables too
            env.extend(task_variables.into_iter().map(|(k, v)| (k, v.to_owned())));
            env
        };

        Some(ResolvedTask {
            id: id.clone(),
            substituted_variables,
            original_task: self.clone(),
            resolved_label: full_label.clone(),
            resolved: SpawnInTerminal {
                id,
                cwd,
                full_label,
                label: human_readable_label,
                command_label: args_with_substitutions.iter().fold(
                    command.clone(),
                    |mut command_label, arg| {
                        command_label.push(' ');
                        command_label.push_str(arg);
                        command_label
                    },
                ),
                command: Some(command),
                args: args_with_substitutions,
                env,
                use_new_terminal: self.use_new_terminal,
                allow_concurrent_runs: self.allow_concurrent_runs,
                reveal: self.reveal,
                reveal_target: self.reveal_target,
                hide: self.hide,
                shell: self.shell.clone(),
                show_summary: self.show_summary,
                show_command: self.show_command,
                show_rerun: true,
                save: self.save,
            },
        })
    }

    /// Validates that all `$ZED_*` variables used in this template are known
    /// variable names, returning a vector with all of the unique unknown
    /// variables.
    ///
    /// Note that `$ZED_CUSTOM_*` variables are never considered to be invalid
    /// since those are provided dynamically by extensions.
    pub fn unknown_variables(&self) -> Vec<String> {
        let mut variables = HashSet::default();

        Self::collect_unknown_variables(&self.label, &mut variables);
        Self::collect_unknown_variables(&self.command, &mut variables);

        self.args
            .iter()
            .for_each(|arg| Self::collect_unknown_variables(arg, &mut variables));

        self.env
            .values()
            .for_each(|value| Self::collect_unknown_variables(value, &mut variables));

        if let Some(cwd) = &self.cwd {
            Self::collect_unknown_variables(cwd, &mut variables);
        }

        variables.into_iter().collect()
    }

    fn collect_unknown_variables(template: &str, unknown: &mut HashSet<String>) {
        shellexpand::env_with_context_no_errors(template, |variable| {
            // It's possible that the variable has a default defined, which is
            // separated by a `:`, for example, `${ZED_FILE:default_value} so we
            // ensure that we're only looking at the variable name itself.
            let colon_position = variable.find(':').unwrap_or(variable.len());
            let variable_name = &variable[..colon_position];

            if variable_name.starts_with(ZED_VARIABLE_NAME_PREFIX)
                && let without_prefix = &variable_name[ZED_VARIABLE_NAME_PREFIX.len()..]
                && !without_prefix.starts_with("CUSTOM_")
                && variable_name.parse::<VariableName>().is_err()
            {
                unknown.insert(variable_name.to_string());
            }

            None::<&str>
        });
    }
}
