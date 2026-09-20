//! ThemeSettings — UI + theme 相关设置。
//!
//! 对齐 Zed `settings_content::ThemeSettingsContent` 的极简版。
//! 用 `#[derive(RegisterSetting)]` 注册到 `settings::SettingsStore`。

use serde::Deserialize;
use serde_json::Value;
use settings::IntoGpui as _;
use settings_macros::RegisterSetting;

/// Customizable settings for the UI and theme system.
#[derive(Clone, PartialEq, Debug, Deserialize, RegisterSetting)]
#[serde(default)]
pub struct ThemeSettings {
    /// Theme selection. 可以是字符串（静态）或 `{ mode, light, dark }` 对象（动态）。
    pub theme: Value,

    /// Icon theme 名称（字符串）。
    pub icon_theme: Option<String>,

    /// UI 字体大小（像素）。
    pub ui_font_size: Option<f32>,

    /// UI 字体族名称。
    pub ui_font_family: Option<String>,

    /// UI 字体粗细（CSS 单位 100-900）。
    pub ui_font_weight: Option<f32>,

    /// 编辑器字体大小（像素）。
    pub buffer_font_size: Option<f32>,

    /// 编辑器字体族名称。
    pub buffer_font_family: Option<String>,

    /// 编辑器字体粗细（CSS 单位 100-900）。
    pub buffer_font_weight: Option<f32>,

    /// 编辑器行高。
    pub buffer_line_height: Option<Value>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            theme: Value::Object({
                let mut m = serde_json::Map::new();
                m.insert("mode".into(), Value::String("system".into()));
                m.insert("light".into(), Value::String("One Light".into()));
                m.insert("dark".into(), Value::String("One Dark".into()));
                m
            }),
            icon_theme: None,
            ui_font_size: None,
            ui_font_family: None,
            ui_font_weight: None,
            buffer_font_size: None,
            buffer_font_family: None,
            buffer_font_weight: None,
            buffer_line_height: None,
        }
    }
}

impl settings::Settings for ThemeSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let t = content.theme.as_ref();
        let default = Self::default();
        Self {
            theme: default.theme,
            icon_theme: None, // IconThemeSelection 暂不转换
            ui_font_size: t.ui_font_size.map(|s| s.0),
            ui_font_family: t.ui_font_family.as_ref().map(|f| f.0.to_string()),
            ui_font_weight: t.ui_font_weight.map(|w| w.0),
            buffer_font_size: t.buffer_font_size.map(|s| s.0),
            buffer_font_family: t.buffer_font_family.as_ref().map(|f| f.0.to_string()),
            buffer_font_weight: t.buffer_font_weight.map(|w| w.0),
            buffer_line_height: default.buffer_line_height,
        }
    }
}
