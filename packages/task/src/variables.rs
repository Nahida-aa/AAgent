use collections::{HashMap, hash_map};

use crate::VariableName;

/// Container for predefined environment variables that describe state of Zed at the time the task was spawned.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct TaskVariables(pub HashMap<VariableName, String>);

impl TaskVariables {
    /// Inserts another variable into the container, overwriting the existing one if it already exists — in this case, the old value is returned.
    pub fn insert(&mut self, variable: VariableName, value: String) -> Option<String> {
        self.0.insert(variable, value)
    }

    /// Extends the container with another one, overwriting the existing variables on collision.
    pub fn extend(&mut self, other: Self) { self.0.extend(other.0); }

    /// Get the value associated with given variable name, if there is one.
    pub fn get(&self, key: &VariableName) -> Option<&str> { self.0.get(key).map(|s| s.as_str()) }

    /// Clear out variables obtained from tree-sitter queries, which are prefixed with '_' character
    pub fn sweep(&mut self) {
        self.0.retain(|name, _| {
            if let VariableName::Custom(name) = name {
                !name.starts_with('_')
            } else {
                true
            }
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VariableName, &String)> { self.0.iter() }
}

impl FromIterator<(VariableName, String)> for TaskVariables {
    fn from_iter<T: IntoIterator<Item = (VariableName, String)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}

impl IntoIterator for TaskVariables {
    type Item = (VariableName, String);
    type IntoIter = hash_map::IntoIter<VariableName, String>;

    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
