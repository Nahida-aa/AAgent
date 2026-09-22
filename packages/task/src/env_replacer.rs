use collections::HashMap;

pub(crate) type VsCodeEnvVariable = String;
pub(crate) type VsCodeCommand = String;
pub(crate) type ZedEnvVariable = String;

pub(crate) struct EnvVariableReplacer {
    variables: HashMap<VsCodeEnvVariable, ZedEnvVariable>,
    commands: HashMap<VsCodeCommand, ZedEnvVariable>,
}

impl EnvVariableReplacer {
    pub(crate) fn new(variables: HashMap<VsCodeEnvVariable, ZedEnvVariable>) -> Self {
        Self {
            variables,
            commands: HashMap::default(),
        }
    }

    pub(crate) fn with_commands(
        mut self,
        commands: impl IntoIterator<Item = (VsCodeCommand, ZedEnvVariable)>,
    ) -> Self {
        self.commands = commands.into_iter().collect();
        self
    }

    pub(crate) fn replace_value(&self, input: serde_json::Value) -> serde_json::Value {
        match input {
            serde_json::Value::String(s) => serde_json::Value::String(self.replace(&s)),
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(|v| self.replace_value(v)).collect())
            }
            serde_json::Value::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (self.replace(&k), self.replace_value(v)))
                    .collect(),
            ),
            _ => input,
        }
    }

    // Replaces occurrences of VsCode-specific environment variables with Zed equivalents.
    pub(crate) fn replace(&self, input: &str) -> String {
        shellexpand::env_with_context_no_errors(&input, |var: &str| {
            // Colons denote a default value in case the variable is not set. We want to preserve that default, as otherwise shellexpand will substitute it for us.
            let colon_position = var.find(':').unwrap_or(var.len());
            let (left, right) = var.split_at(colon_position);
            if left == "env" && !right.is_empty() {
                let variable_name = &right[1..];
                return Some(format!("${{{variable_name}}}"));
            } else if left == "command" && !right.is_empty() {
                let command_name = &right[1..];
                if let Some(replacement_command) = self.commands.get(command_name) {
                    return Some(format!("${{{replacement_command}}}"));
                }
            }

            let (variable_name, default) = (left, right);
            let append_previous_default = |ret: &mut String| {
                if !default.is_empty() {
                    ret.push_str(default);
                }
            };
            if let Some(substitution) = self.variables.get(variable_name) {
                // Got a VSCode->Zed hit, perform a substitution
                let mut name = format!("${{{substitution}");
                append_previous_default(&mut name);
                name.push('}');
                return Some(name);
            }
            // This is an unknown variable.
            // We should not error out, as they may come from user environment (e.g. $PATH). That means that the variable substitution might not be perfect.
            // If there's a default, we need to return the string verbatim as otherwise shellexpand will apply that default for us.
            if !default.is_empty() {
                return Some(format!("${{{var}}}"));
            }
            // Else we can just return None and that variable will be left as is.
            None
        })
        .into_owned()
    }
}
