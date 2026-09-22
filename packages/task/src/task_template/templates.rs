use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use util::schemars::{AllowTrailingCommas, DefaultDenyUnknownFields};

use super::template::TaskTemplate;

/// A group of Tasks defined in a JSON file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskTemplates(pub Vec<TaskTemplate>);

impl TaskTemplates {
    pub const FILE_NAME: &str = "tasks.json";

    /// Generates JSON schema of Tasks JSON template format.
    pub fn generate_json_schema() -> serde_json::Value {
        let schema = schemars::generate::SchemaSettings::draft2019_09()
            .with_transform(DefaultDenyUnknownFields)
            .with_transform(AllowTrailingCommas)
            .into_generator()
            .root_schema_for::<Self>();

        serde_json::to_value(schema).unwrap()
    }
}
