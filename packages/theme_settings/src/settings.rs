//! ThemeSettings — UI + theme 相关设置。
//!
//! 对齐 Zed `settings_content::ThemeSettingsContent` 的极简版。
//! 用 `#[derive(RegisterSetting)]` 注册到 `settings::SettingsStore`。

use gpui::{App, Font, FontFeatures, FontStyle, FontWeight, Pixels, SharedString, px};
use serde::Deserialize;
use serde_json::Value;
use settings::IntoGpui as _;
use settings_macros::RegisterSetting;

/// 没有显式设置时的默认字号/字体族（zed 的默认值；如需不同请改这里）。
const DEFAULT_BUFFER_FONT_SIZE: f32 = 15.0;
const DEFAULT_UI_FONT_SIZE: f32 = 16.0;
const DEFAULT_FONT_FAMILY: &str = ".SystemUIFont";

fn font_from(family: Option<&str>, weight: Option<f32>) -> Font {
    Font {
        family: family.unwrap_or(DEFAULT_FONT_FAMILY).into(),
        weight: FontWeight(weight.unwrap_or(400.0)),
        style: FontStyle::default(),
        features: FontFeatures::default(),
        fallbacks: None,
    }
}

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

    /// 编辑器字体（对齐 zed `ThemeSettings::buffer_font`）。
    #[serde(skip)]
    pub buffer_font: Font,

    /// UI 字体（对齐 zed `ThemeSettings::ui_font`）。
    #[serde(skip)]
    pub ui_font: Font,
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
            buffer_font: font_from(None, None),
            ui_font: font_from(None, None),
        }
    }
}

impl ThemeSettings {
    pub fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(self.buffer_font_size.unwrap_or(DEFAULT_BUFFER_FONT_SIZE))
    }

    pub fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(self.ui_font_size.unwrap_or(DEFAULT_UI_FONT_SIZE))
    }

    /// agent 面板里的编辑器字号（暂与 `buffer_font_size` 一致）。
    pub fn agent_buffer_font_size(&self, cx: &App) -> Pixels { self.buffer_font_size(cx) }

    /// agent 面板里的 UI 字号（暂与 `ui_font_size` 一致）。
    pub fn agent_ui_font_size(&self, cx: &App) -> Pixels { self.ui_font_size(cx) }

    /// markdown 预览字号（暂与 `ui_font_size` 一致）。
    pub fn markdown_preview_font_size(&self, cx: &App) -> Pixels { self.ui_font_size(cx) }

    pub fn buffer_font_family(&self) -> &SharedString { &self.buffer_font.family }

    pub fn ui_font_family(&self) -> &SharedString { &self.ui_font.family }

    pub fn agent_buffer_font_family(&self) -> &SharedString { &self.buffer_font.family }

    pub fn agent_ui_font_family(&self) -> &SharedString { &self.ui_font.family }

    pub fn markdown_preview_font_family(&self) -> &SharedString { &self.ui_font.family }

    pub fn markdown_preview_code_font_family(&self) -> &SharedString { &self.buffer_font.family }

    /// 行高倍数；无显式设置时取 1.5。
    pub fn buffer_line_height_value(&self) -> f32 {
        match &self.buffer_line_height {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(1.5) as f32,
            _ => 1.5,
        }
    }
}

impl settings::Settings for ThemeSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let t = content.theme.as_ref();
        let default = Self::default();

        let ui_font_family = t.ui_font_family.as_ref().map(|f| f.0.to_string());
        let ui_font_weight = t.ui_font_weight.map(|w| w.0);
        let buffer_font_family = t.buffer_font_family.as_ref().map(|f| f.0.to_string());
        let buffer_font_weight = t.buffer_font_weight.map(|w| w.0);

        Self {
            theme: default.theme,
            icon_theme: None, // IconThemeSelection 暂不转换
            ui_font_size: t.ui_font_size.map(|s| s.0),
            ui_font_family: ui_font_family.clone(),
            ui_font_weight,
            buffer_font_size: t.buffer_font_size.map(|s| s.0),
            buffer_font_family: buffer_font_family.clone(),
            buffer_font_weight,
            buffer_line_height: default.buffer_line_height,
            buffer_font: font_from(buffer_font_family.as_deref(), buffer_font_weight),
            ui_font: font_from(ui_font_family.as_deref(), ui_font_weight),
        }
    }
}
