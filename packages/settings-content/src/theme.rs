//! Theme 相关的 settings 内容类型。
//!
//! 对齐 Zed `crates/settings_content/src/theme.rs`。
//!
//! 这里放**从 JSON 反序列化的数据结构**（settings_content 层），
//! 运行时主题类型（`Theme`, `ThemeStyles`, `ActiveTheme` ...）在
//! `aa-gpui-kit-theme` crate（gpui_learn）。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

// ---------- 常量 ----------

pub const DEFAULT_LIGHT_THEME: &str = "One Light";
pub const DEFAULT_DARK_THEME: &str = "One Dark";

// ---------- ThemeName ----------

/// Theme 名称（transparent newtype，包 `Arc<str>`）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThemeName(pub Arc<str>);

impl From<String> for ThemeName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for ThemeName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

// ---------- IconThemeName ----------

/// Icon theme 名称。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IconThemeName(pub Arc<str>);

impl From<String> for IconThemeName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for IconThemeName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

// ---------- ThemeAppearanceMode ----------

/// 选择主题时用的模式。
///
/// - `Light` / `Dark` — 固定选对应主题
/// - `System` — 跟随系统明暗
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAppearanceMode {
    Light,
    Dark,
    #[default]
    System,
}

// ---------- ThemeSelection ----------

/// 主题选择 — 可以是静态（单个主题）或动态（按明暗切换）。
///
/// JSON 格式：
/// - 静态：`"theme": "One Dark"`
/// - 动态：`"theme": { "mode": "system", "light": "One Light", "dark": "One Dark" }`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ThemeSelection {
    /// 静态主题选择。
    Static(ThemeName),
    /// 动态主题选择（按 mode 切换 light / dark）。
    Dynamic {
        #[serde(default)]
        mode: ThemeAppearanceMode,
        light: ThemeName,
        dark: ThemeName,
    },
}

impl Default for ThemeSelection {
    fn default() -> Self {
        Self::Dynamic {
            mode: ThemeAppearanceMode::default(),
            light: ThemeName::from(DEFAULT_LIGHT_THEME),
            dark: ThemeName::from(DEFAULT_DARK_THEME),
        }
    }
}

/// 图标主题选择 — 同 ThemeSelection 形状。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IconThemeSelection {
    Static(IconThemeName),
    Dynamic {
        #[serde(default)]
        mode: ThemeAppearanceMode,
        light: IconThemeName,
        dark: IconThemeName,
    },
}

impl Default for IconThemeSelection {
    fn default() -> Self {
        Self::Static(IconThemeName::default())
    }
}

// ---------- FontSize ----------

/// 字体大小（像素）。
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontSize(pub f32);

impl From<f32> for FontSize {
    fn from(v: f32) -> Self {
        Self(v)
    }
}

impl From<FontSize> for f32 {
    fn from(v: FontSize) -> Self {
        v.0
    }
}

// ---------- FontFamilyName ----------

/// 字体族名称（包 `Arc<str>`）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontFamilyName(pub Arc<str>);

impl From<String> for FontFamilyName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for FontFamilyName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

// ---------- FontWeightContent ----------

/// 字体粗细（CSS 单位 100-900）。
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontWeightContent(pub f32);

impl FontWeightContent {
    pub const THIN: Self = Self(100.0);
    pub const EXTRA_LIGHT: Self = Self(200.0);
    pub const LIGHT: Self = Self(300.0);
    pub const NORMAL: Self = Self(400.0);
    pub const MEDIUM: Self = Self(500.0);
    pub const SEMIBOLD: Self = Self(600.0);
    pub const BOLD: Self = Self(700.0);
    pub const EXTRA_BOLD: Self = Self(800.0);
    pub const BLACK: Self = Self(900.0);
}

// ---------- BufferLineHeight ----------

/// 编辑器行高。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferLineHeight {
    #[default]
    Comfortable,
    Standard,
    Custom(f32),
}

// ---------- UiDensity ----------

/// UI 密度（实验性）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    #[serde(alias = "compact")]
    Compact,
    #[default]
    #[serde(alias = "default")]
    Default,
    #[serde(alias = "comfortable")]
    Comfortable,
}

impl UiDensity {
    pub fn spacing_ratio(self) -> f32 {
        match self {
            Self::Compact => 0.75,
            Self::Default => 1.0,
            Self::Comfortable => 1.25,
        }
    }
}

// ---------- ThemeSettingsContent ----------

/// Theme 相关的所有 settings 字段。
///
/// 对齐 Zed `settings_content::ThemeSettingsContent` 的核心子集。
/// 完整字段列表见 Zed 源码，这里先放 AAgent 需要的。
///
/// 所有字段都是 `Option<_>`，None 表示用默认值（由 `theme-settings` crate 的 Default impl 或
/// gpui_learn 的 ThemeRegistry 提供）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettingsContent {
    /// UI 字体大小。
    pub ui_font_size: Option<FontSize>,
    /// UI 字体族。
    pub ui_font_family: Option<FontFamilyName>,
    /// UI 字体粗细（100-900）。
    pub ui_font_weight: Option<FontWeightContent>,

    /// 编辑器字体族。
    pub buffer_font_family: Option<FontFamilyName>,
    /// 编辑器字体大小。
    pub buffer_font_size: Option<FontSize>,
    /// 编辑器字体粗细（100-900）。
    pub buffer_font_weight: Option<FontWeightContent>,
    /// 编辑器行高。
    pub buffer_line_height: Option<BufferLineHeight>,

    /// 主题选择。
    pub theme: Option<ThemeSelection>,
    /// 图标主题选择。
    pub icon_theme: Option<IconThemeSelection>,

    /// UI 密度（experimental）。
    #[serde(rename = "unstable.ui_density")]
    pub ui_density: Option<UiDensity>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_theme_selection_static() {
        let s: ThemeSelection = serde_json::from_value(json!("One Dark")).unwrap();
        assert!(matches!(s, ThemeSelection::Static(_)));
    }

    #[test]
    fn test_theme_selection_dynamic() {
        let s: ThemeSelection = serde_json::from_value(json!({
            "mode": "system",
            "light": "One Light",
            "dark": "One Dark"
        }))
        .unwrap();
        assert!(matches!(s, ThemeSelection::Dynamic { .. }));
    }

    #[test]
    fn test_theme_settings_default() {
        let s: ThemeSettingsContent = serde_json::from_value(json!({})).unwrap();
        assert!(s.theme.is_none());
        assert!(s.buffer_font_size.is_none());
    }
}
