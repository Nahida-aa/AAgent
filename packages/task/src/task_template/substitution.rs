use anyhow::{Context as _, bail};
use collections::{HashMap, HashSet};
use serde::Serialize;
use sha2::{Digest, Sha256};
use util::truncate_and_remove_front;

use crate::{TaskContext, VariableName, ZED_VARIABLE_NAME_PREFIX};

const MAX_DISPLAY_VARIABLE_LENGTH: usize = 15;

pub(crate) fn truncate_variables(
    task_variables: &HashMap<String, &str>,
) -> HashMap<String, String> {
    task_variables
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                truncate_and_remove_front(value, MAX_DISPLAY_VARIABLE_LENGTH),
            )
        })
        .collect()
}

pub(crate) fn to_hex_hash(object: impl Serialize) -> anyhow::Result<String> {
    let json = serde_json_lenient::to_string(&object).context("serializing the object")?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

pub fn substitute_variables_in_str(template_str: &str, context: &TaskContext) -> Option<String> {
    let mut variable_names = HashMap::default();
    let mut substituted_variables = HashSet::default();
    let task_variables = context
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
    substitute_all_template_variables_in_str(
        template_str,
        &task_variables,
        &variable_names,
        &mut substituted_variables,
    )
}

pub(crate) fn substitute_all_template_variables_in_str<A: AsRef<str>>(
    template_str: &str,
    task_variables: &HashMap<String, A>,
    variable_names: &HashMap<String, VariableName>,
    substituted_variables: &mut HashSet<VariableName>,
) -> Option<String> {
    let substituted_string = shellexpand::env_with_context(template_str, |var| {
        // Colons denote a default value in case the variable is not set. We
        // want to preserve that default, as otherwise shellexpand will
        // substitute it for us.
        let colon_position = var.find(':').unwrap_or(var.len());
        let (variable_name, default) = var.split_at(colon_position);
        if let Some(name) = task_variables.get(variable_name) {
            if let Some(substituted_variable) = variable_names.get(variable_name) {
                substituted_variables.insert(substituted_variable.clone());
            }
            // Got a task variable hit - use the variable value, ignore default
            return Ok(Some(name.as_ref().to_owned()));
        } else if variable_name.starts_with(ZED_VARIABLE_NAME_PREFIX) {
            // Unknown ZED variable - use default if available
            if !default.is_empty() {
                // Strip the colon and return the default value
                return Ok(Some(default[1..].to_owned()));
            } else {
                bail!("Unknown variable name: {variable_name}");
            }
        }
        // This is an unknown variable.
        // We should not error out, as they may come from user environment (e.g.
        // $PATH). That means that the variable substitution might not be
        // perfect. If there's a default, we need to return the string verbatim
        // as otherwise shellexpand will apply that default for us.
        if !default.is_empty() {
            return Ok(Some(format!("${{{var}}}")));
        }

        // Else we can just return None and that variable will be left as is.
        Ok(None)
    })
    .ok()?;

    Some(substituted_string.into_owned())
}

pub(crate) fn substitute_all_template_variables_in_vec(
    template_strs: &[String],
    task_variables: &HashMap<String, &str>,
    variable_names: &HashMap<String, VariableName>,
    substituted_variables: &mut HashSet<VariableName>,
) -> Option<Vec<String>> {
    let mut expanded = Vec::with_capacity(template_strs.len());
    for variable in template_strs {
        let new_value = substitute_all_template_variables_in_str(
            variable,
            task_variables,
            variable_names,
            substituted_variables,
        )?;

        if !new_value.is_empty() || variable.is_empty() {
            expanded.push(new_value);
        }
    }

    Some(expanded)
}

pub fn substitute_variables_in_map(
    keys_and_values: &HashMap<String, String>,
    context: &TaskContext,
) -> Option<HashMap<String, String>> {
    let mut variable_names = HashMap::default();
    let mut substituted_variables = HashSet::default();
    let task_variables = context
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
    substitute_all_template_variables_in_map(
        keys_and_values,
        &task_variables,
        &variable_names,
        &mut substituted_variables,
    )
}

pub(crate) fn substitute_all_template_variables_in_map(
    keys_and_values: &HashMap<String, String>,
    task_variables: &HashMap<String, &str>,
    variable_names: &HashMap<String, VariableName>,
    substituted_variables: &mut HashSet<VariableName>,
) -> Option<HashMap<String, String>> {
    let mut new_map: HashMap<String, String> = Default::default();
    for (key, value) in keys_and_values {
        let new_value = substitute_all_template_variables_in_str(
            value,
            task_variables,
            variable_names,
            substituted_variables,
        )?;
        let new_key = substitute_all_template_variables_in_str(
            key,
            task_variables,
            variable_names,
            substituted_variables,
        )?;
        new_map.insert(new_key, new_value);
    }

    Some(new_map)
}
