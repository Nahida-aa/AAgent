use std::sync::Arc;

use gpui::App;
use language::Buffer;
use settings::{Settings as _, SettingsLocation};

/// AI 功能的全局开关（对齐 Zed `project::DisableAiSettings`）。
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

impl DisableAiSettings {
    /// Returns whether AI is disabled for the given buffer。
    ///
    /// 按 buffer 所在文件定位 worktree + path，读取该位置的 DisableAiSettings。
    /// 对齐 Zed `project::DisableAiSettings::is_ai_disabled_for_buffer`。
    pub fn is_ai_disabled_for_buffer(buffer: Option<&gpui::Entity<Buffer>>, cx: &App) -> bool {
        Self::is_ai_disabled_for_file(
            buffer.and_then(|buffer| buffer.read(cx).file()),
            cx,
        )
    }

    pub fn is_ai_disabled_for_file(
        file: Option<&Arc<dyn language::File>>,
        cx: &App,
    ) -> bool {
        let location = file.map(|f| SettingsLocation {
            worktree_id: f.worktree_id(cx),
            path: f.path().as_ref(),
        });
        Self::get(location, cx).disable_ai
    }
}
