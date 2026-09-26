//! ThemeSettings — UI + theme 相关设置。
//!
//! 对齐 Zed `settings_content::ThemeSettingsContent` 的极简版。
//! 用 `#[derive(RegisterSetting)]` 注册到 `settings::SettingsStore`。

use gpui::{App, Font, FontFeatures, FontStyle, FontWeight, Pixels, SharedString, px};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use settings::IntoGpui as _;
use settings_macros::RegisterSetting;

use aa_gpui_kit_theme::{Appearance, DEFAULT_ICON_THEME_NAME};

// 对齐 zed `theme_settings/src/settings.rs:11`：选择类类型由 settings_content
// 定义，这里只重导出 —— 照搬 zed 的 `theme_settings::ThemeAppearanceMode::Light`
// 才能直接过。
pub use settings_content::{IconThemeName, ThemeAppearanceMode, ThemeName};

/// 运行时主题选择（对齐 zed `theme_settings/src/settings.rs:142`）。
///
/// 与 `settings_content::ThemeSelection` 是**两个类型**：那边是 JSON 的反序列化
/// 形状（跟着用户 settings.json 走），这边是装配后的运行时结果。zed 也这么分。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeSelection {
    /// 固定一个主题。
    Static(ThemeName),
    /// 按明暗切换。
    Dynamic {
        #[serde(default)]
        mode: ThemeAppearanceMode,
        light: ThemeName,
        dark: ThemeName,
    },
}

impl From<settings_content::ThemeSelection> for ThemeSelection {
    fn from(selection: settings_content::ThemeSelection) -> Self {
        match selection {
            settings_content::ThemeSelection::Static(theme) => Self::Static(theme),
            settings_content::ThemeSelection::Dynamic { mode, light, dark } => {
                Self::Dynamic { mode, light, dark }
            }
        }
    }
}

impl Default for ThemeSelection {
    fn default() -> Self {
        Self::Dynamic {
            mode: ThemeAppearanceMode::System,
            light: ThemeName::from(settings_content::DEFAULT_LIGHT_THEME),
            dark: ThemeName::from(settings_content::DEFAULT_DARK_THEME),
        }
    }
}

impl ThemeSelection {
    /// 按系统明暗解析出主题名（对齐 zed `settings.rs:172`）。
    pub fn name(&self, system_appearance: Appearance) -> ThemeName {
        match self {
            Self::Static(theme) => theme.clone(),
            Self::Dynamic { mode, light, dark } => match mode {
                ThemeAppearanceMode::Light => light.clone(),
                ThemeAppearanceMode::Dark => dark.clone(),
                ThemeAppearanceMode::System => match system_appearance {
                    Appearance::Light => light.clone(),
                    Appearance::Dark => dark.clone(),
                },
            },
        }
    }

    /// 当前模式；`Static` 没有模式概念 → `None`（对齐 zed `settings.rs:188`）。
    pub fn mode(&self) -> Option<ThemeAppearanceMode> {
        match self {
            Self::Static(_) => None,
            Self::Dynamic { mode, .. } => Some(*mode),
        }
    }
}

/// 运行时图标主题选择（对齐 zed `theme_settings/src/settings.rs:196`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconThemeSelection {
    Static(IconThemeName),
    Dynamic {
        mode: ThemeAppearanceMode,
        light: IconThemeName,
        dark: IconThemeName,
    },
}

impl From<settings_content::IconThemeSelection> for IconThemeSelection {
    fn from(selection: settings_content::IconThemeSelection) -> Self {
        match selection {
            settings_content::IconThemeSelection::Static(theme) => Self::Static(theme),
            settings_content::IconThemeSelection::Dynamic { mode, light, dark } => {
                Self::Dynamic { mode, light, dark }
            }
        }
    }
}

impl IconThemeSelection {
    /// 按系统明暗解析出图标主题名（对齐 zed `settings.rs:225`）。
    pub fn name(&self, system_appearance: Appearance) -> IconThemeName {
        match self {
            Self::Static(theme) => theme.clone(),
            Self::Dynamic { mode, light, dark } => match mode {
                ThemeAppearanceMode::Light => light.clone(),
                ThemeAppearanceMode::Dark => dark.clone(),
                ThemeAppearanceMode::System => match system_appearance {
                    Appearance::Light => light.clone(),
                    Appearance::Dark => dark.clone(),
                },
            },
        }
    }

    pub fn mode(&self) -> Option<ThemeAppearanceMode> {
        match self {
            Self::Static(_) => None,
            Self::Dynamic { mode, .. } => Some(*mode),
        }
    }
}

/// 切换主题的明暗模式（对齐 zed `theme_settings/src/settings.rs:316`）。
///
/// 直接改 `SettingsContent`，调用点配合 `settings::update_settings_file` 落盘。
pub fn set_mode(content: &mut settings_content::SettingsContent, mode: ThemeAppearanceMode) {
    let theme = content.theme.as_mut();

    if let Some(selection) = theme.theme.as_mut() {
        match selection {
            settings_content::ThemeSelection::Static(_) => {
                *selection = settings_content::ThemeSelection::Dynamic {
                    mode: ThemeAppearanceMode::System,
                    light: ThemeName::from(settings_content::DEFAULT_LIGHT_THEME),
                    dark: ThemeName::from(settings_content::DEFAULT_DARK_THEME),
                };
            }
            settings_content::ThemeSelection::Dynamic {
                mode: mode_to_update,
                ..
            } => *mode_to_update = mode,
        }
    } else {
        theme.theme = Some(settings_content::ThemeSelection::Dynamic {
            mode,
            light: ThemeName::from(settings_content::DEFAULT_LIGHT_THEME),
            dark: ThemeName::from(settings_content::DEFAULT_DARK_THEME),
        });
    }

    if let Some(selection) = theme.icon_theme.as_mut() {
        match selection {
            settings_content::IconThemeSelection::Static(icon_theme) => {
                *selection = settings_content::IconThemeSelection::Dynamic {
                    mode,
                    light: icon_theme.clone(),
                    dark: icon_theme.clone(),
                };
            }
            settings_content::IconThemeSelection::Dynamic {
                mode: mode_to_update,
                ..
            } => *mode_to_update = mode,
        }
    } else {
        theme.icon_theme = Some(settings_content::IconThemeSelection::Static(
            IconThemeName::from(DEFAULT_ICON_THEME_NAME),
        ));
    }
}

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
    /// 主题选择（运行时类型；JSON 形状见 [`settings_content::ThemeSelection`]）。
    pub theme: ThemeSelection,

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
            theme: ThemeSelection::default(),
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
            theme: t
                .theme
                .clone()
                .map(ThemeSelection::from)
                .unwrap_or_default(),
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
