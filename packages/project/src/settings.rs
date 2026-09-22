use settings::Settings as _;

pub struct DisableAiSettings {
    pub disable_ai: bool,
}

impl settings::Settings for DisableAiSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        Self {
            disable_ai: content.project.disable_ai.unwrap().0,
        }
    }
}
