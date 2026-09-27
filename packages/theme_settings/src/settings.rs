//! ThemeSettings — UI + theme 相关设置。
//!
//! 对齐 Zed `settings_content::ThemeSettingsContent` 的运行时版本。
//! 用 `#[derive(RegisterSetting)]` 注册到 `settings::SettingsStore`。

use std::collections::HashMap;
use std::sync::Arc;

use aa_gpui_kit_theme::{Appearance, BufferLineHeight, DEFAULT_ICON_THEME_NAME, UiDensity};
use gpui::{
    App, Context, Font, FontFeatures, FontStyle, Global, Pixels, SharedString, Subscription,
    Window, px,
};
use serde::{Deserialize, Serialize};
use settings::IntoGpui as _;
use settings_macros::RegisterSetting;

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

impl Default for IconThemeSelection {
    fn default() -> Self {
        Self::Static(IconThemeName::from(DEFAULT_ICON_THEME_NAME))
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

// ---------- 字体大小全局覆盖 ----------

const MIN_FONT_SIZE: Pixels = px(6.0);
const MAX_FONT_SIZE: Pixels = px(100.0);

#[derive(Default)]
pub struct BufferFontSize(Pixels);
impl Global for BufferFontSize {}

#[derive(Default)]
pub struct UiFontSize(Pixels);
impl Global for UiFontSize {}

#[derive(Default)]
pub struct AgentUiFontSize(Pixels);
impl Global for AgentUiFontSize {}

#[derive(Default)]
pub struct AgentBufferFontSize(Pixels);
impl Global for AgentBufferFontSize {}

#[derive(Default)]
pub struct GitCommitBufferFontSize(Pixels);
impl Global for GitCommitBufferFontSize {}

#[derive(Default)]
pub struct MarkdownPreviewFontSize(Pixels);
impl Global for MarkdownPreviewFontSize {}

/// Ensures font size is within the valid range.
pub fn clamp_font_size(size: Pixels) -> Pixels {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

// ---------- ThemeSettings ----------

fn font_from(family: Option<&str>, weight: Option<f32>) -> Font {
    Font {
        family: family
            .unwrap_or(".SystemUIFont")
            .to_string()
            .into(),
        weight: gpui::FontWeight(weight.unwrap_or(400.0)),
        style: FontStyle::default(),
        features: FontFeatures::default(),
        fallbacks: None,
    }
}

/// Customizable settings for the UI and theme system.
///
/// 对齐 Zed `crates/theme_settings/src/settings.rs` 的 ThemeSettings。
///
/// 注意：字段分两类 —— 运行时可全局覆盖的（ui_font_size/buffer_font_size 等）和
/// 从 settings 文件直接解析的（theme 等）。后者需要 serde Deserialize，
/// 前者在 from_settings 里手动装配，用 `#[serde(skip)]`。
#[derive(Clone, PartialEq, Debug, Deserialize, RegisterSetting)]
#[serde(default)]
pub struct ThemeSettings {
    /// The UI font size (可被全局 UiFontSize 覆盖).
    ui_font_size: Pixels,

    /// The font used for UI elements（从 settings 装配，非 JSON 直出）。
    #[serde(skip)]
    pub ui_font: Font,

    /// The font size used for buffers (可被全局 BufferFontSize 覆盖).
    buffer_font_size: Pixels,

    /// The font used for buffers（从 settings 装配，非 JSON 直出）。
    #[serde(skip)]
    pub buffer_font: Font,

    /// The agent UI font family. Falls back to the UI font family if unset.
    agent_ui_font_family: Option<SharedString>,

    /// The agent font size. Falls back to the UI font size if unset.
    agent_ui_font_size: Option<Pixels>,

    /// The agent buffer font family. Falls back to the buffer font family if unset.
    agent_buffer_font_family: Option<SharedString>,

    /// The agent buffer font size.
    agent_buffer_font_size: Option<Pixels>,

    git_commit_buffer_font_size: Option<Pixels>,

    /// The font family to use for rendering in the markdown preview.
    markdown_preview_font_family: Option<SharedString>,

    /// The font family to use for code in the markdown preview.
    markdown_preview_code_font_family: Option<SharedString>,

    /// The font size to use for rendering in the markdown preview.
    markdown_preview_font_size: Option<Pixels>,

    /// The theme to use for the markdown preview.
    pub markdown_preview_theme: Option<ThemeSelection>,

    /// The line height for buffers, and the terminal.
    /// 从 settings_content 读，转换为 aa_gpui_kit_theme::BufferLineHeight（有 .value()）。
    #[serde(skip)]
    pub buffer_line_height: BufferLineHeight,

    /// The current theme selection.
    pub theme: ThemeSelection,

    /// The current icon theme selection.
    #[serde(skip)]
    pub icon_theme: IconThemeSelection,

    /// Manual overrides for the active theme (experimental).
    pub experimental_theme_overrides: Option<settings::ThemeStyleContent>,

    /// Manual overrides per theme.
    pub theme_overrides: HashMap<String, settings::ThemeStyleContent>,

    /// The density of the UI.
    pub ui_density: UiDensity,

    /// The amount of fading applied to unnecessary code.
    pub unnecessary_code_fade: f32,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            ui_font_size: px(16.0),
            ui_font: font_from(None, None),
            buffer_font_size: px(15.0),
            buffer_font: font_from(None, None),
            agent_ui_font_family: None,
            agent_ui_font_size: None,
            agent_buffer_font_family: None,
            agent_buffer_font_size: None,
            git_commit_buffer_font_size: None,
            markdown_preview_font_family: None,
            markdown_preview_code_font_family: None,
            markdown_preview_font_size: None,
            markdown_preview_theme: None,
            buffer_line_height: BufferLineHeight::default(),
            theme: ThemeSelection::default(),
            icon_theme: IconThemeSelection::default(),
            experimental_theme_overrides: None,
            theme_overrides: HashMap::default(),
            ui_density: UiDensity::default(),
            unnecessary_code_fade: 0.5,
        }
    }
}

impl ThemeSettings {
    /// Returns the buffer font size（考虑全局覆盖）。
    pub fn buffer_font_size(&self, cx: &App) -> Pixels {
        let font_size = cx
            .try_global::<BufferFontSize>()
            .map(|size| size.0)
            .unwrap_or(self.buffer_font_size);
        clamp_font_size(font_size)
    }

    /// Returns the UI font size（考虑全局覆盖）。
    pub fn ui_font_size(&self, cx: &App) -> Pixels {
        let font_size = cx
            .try_global::<UiFontSize>()
            .map(|size| size.0)
            .unwrap_or(self.ui_font_size);
        clamp_font_size(font_size)
    }

    /// Returns the agent panel font size. Falls back to the UI font size if unset.
    pub fn agent_ui_font_size(&self, cx: &App) -> Pixels {
        cx.try_global::<AgentUiFontSize>()
            .map(|size| size.0)
            .or(self.agent_ui_font_size)
            .map(clamp_font_size)
            .unwrap_or_else(|| self.ui_font_size(cx))
    }

    pub fn agent_ui_font_family(&self) -> &SharedString {
        self.agent_ui_font_family
            .as_ref()
            .unwrap_or(&self.ui_font.family)
    }

    /// Returns the agent panel buffer font size.
    pub fn agent_buffer_font_size(&self, cx: &App) -> Pixels {
        cx.try_global::<AgentBufferFontSize>()
            .map(|size| size.0)
            .or(self.agent_buffer_font_size)
            .map(clamp_font_size)
            .unwrap_or_else(|| self.buffer_font_size(cx))
    }

    pub fn agent_buffer_font_family(&self) -> &SharedString {
        self.agent_buffer_font_family
            .as_ref()
            .unwrap_or(&self.buffer_font.family)
    }

    pub fn git_commit_buffer_font_size(&self, cx: &App) -> Pixels {
        cx.try_global::<GitCommitBufferFontSize>()
            .map(|size| size.0)
            .or(self.git_commit_buffer_font_size)
            .map(clamp_font_size)
            .unwrap_or_else(|| self.buffer_font_size(cx))
    }

    /// Returns the font family to use in the markdown preview,
    /// falling back to the UI font family when unset.
    pub fn markdown_preview_font_family(&self) -> &SharedString {
        self.markdown_preview_font_family
            .as_ref()
            .unwrap_or(&self.ui_font.family)
    }

    /// Returns the font family to use for code in the markdown preview,
    /// falling back to the buffer font family when unset.
    pub fn markdown_preview_code_font_family(&self) -> &SharedString {
        self.markdown_preview_code_font_family
            .as_ref()
            .unwrap_or(&self.buffer_font.family)
    }

    /// Returns the markdown preview font size.
    ///
    /// Note: the fallback deliberately uses `self.ui_font_size` instead of `ui_font_size(cx)`,
    /// so that temporary UI zoom does not also resize the markdown preview.
    pub fn markdown_preview_font_size(&self, cx: &App) -> Pixels {
        cx.try_global::<MarkdownPreviewFontSize>()
            .map(|size| size.0)
            .or(self.markdown_preview_font_size)
            .map(clamp_font_size)
            .unwrap_or_else(|| clamp_font_size(self.ui_font_size))
    }

    /// Returns the buffer font size, read from the settings (不考虑全局覆盖).
    pub fn buffer_font_size_settings(&self) -> Pixels {
        self.buffer_font_size
    }

    /// Returns the UI font size, read from the settings (不考虑全局覆盖).
    pub fn ui_font_size_settings(&self) -> Pixels {
        self.ui_font_size
    }

    pub fn agent_ui_font_size_settings(&self) -> Option<Pixels> {
        self.agent_ui_font_size
    }

    pub fn agent_buffer_font_size_settings(&self) -> Option<Pixels> {
        self.agent_buffer_font_size
    }

    pub fn git_commit_buffer_font_size_settings(&self) -> Option<Pixels> {
        self.git_commit_buffer_font_size
    }

    pub fn markdown_preview_font_size_settings(&self) -> Option<Pixels> {
        self.markdown_preview_font_size
    }

    /// Returns the buffer's line height.
    pub fn line_height(&self) -> f32 {
        let v = self.buffer_line_height.value();
        v.max(1.0)
    }
}

// ---------- 调整字号的全局函数 ----------

/// Observe changes to the adjusted buffer font size.
pub fn observe_buffer_font_size_adjustment<V: 'static>(
    cx: &mut Context<V>,
    f: impl 'static + Fn(&mut V, &mut Context<V>),
) -> Subscription {
    cx.observe_global::<BufferFontSize>(f)
}

/// Adjusts the buffer font size, without persisting the result in the settings.
pub fn adjust_buffer_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let buffer_font_size = ThemeSettings::get_global(cx).buffer_font_size;
    let adjusted_size = cx
        .try_global::<BufferFontSize>()
        .map_or(buffer_font_size, |adjusted_size| adjusted_size.0);
    cx.set_global(BufferFontSize(clamp_font_size(f(adjusted_size))));
    cx.refresh_windows();
}

/// Resets the buffer font size to the default value.
pub fn reset_buffer_font_size(cx: &mut App) {
    if cx.has_global::<BufferFontSize>() {
        cx.remove_global::<BufferFontSize>();
        cx.refresh_windows();
    }
}

/// Increases the buffer font size by 1 pixel.
pub fn increase_buffer_font_size(cx: &mut App) {
    adjust_buffer_font_size(cx, |size| size + px(1.0));
}

/// Decreases the buffer font size by 1 pixel.
pub fn decrease_buffer_font_size(cx: &mut App) {
    adjust_buffer_font_size(cx, |size| size - px(1.0));
}

/// Sets the adjusted UI font size.
pub fn adjust_ui_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let ui_font_size = ThemeSettings::get_global(cx).ui_font_size(cx);
    let adjusted_size = cx
        .try_global::<UiFontSize>()
        .map_or(ui_font_size, |adjusted_size| adjusted_size.0);
    cx.set_global(UiFontSize(clamp_font_size(f(adjusted_size))));
    cx.refresh_windows();
}

/// Resets the UI font size to the default value.
pub fn reset_ui_font_size(cx: &mut App) {
    if cx.has_global::<UiFontSize>() {
        cx.remove_global::<UiFontSize>();
        cx.refresh_windows();
    }
}

pub fn adjust_agent_ui_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let size = ThemeSettings::get_global(cx).agent_ui_font_size(cx);
    let adjusted = cx
        .try_global::<AgentUiFontSize>()
        .map_or(size, |s| s.0);
    cx.set_global(AgentUiFontSize(clamp_font_size(f(adjusted))));
    cx.refresh_windows();
}

pub fn reset_agent_ui_font_size(cx: &mut App) {
    if cx.has_global::<AgentUiFontSize>() {
        cx.remove_global::<AgentUiFontSize>();
        cx.refresh_windows();
    }
}

pub fn adjust_agent_buffer_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let size = ThemeSettings::get_global(cx).agent_buffer_font_size(cx);
    let adjusted = cx
        .try_global::<AgentBufferFontSize>()
        .map_or(size, |s| s.0);
    cx.set_global(AgentBufferFontSize(clamp_font_size(f(adjusted))));
    cx.refresh_windows();
}

pub fn reset_agent_buffer_font_size(cx: &mut App) {
    if cx.has_global::<AgentBufferFontSize>() {
        cx.remove_global::<AgentBufferFontSize>();
        cx.refresh_windows();
    }
}

pub fn adjust_git_commit_buffer_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let size = ThemeSettings::get_global(cx).git_commit_buffer_font_size(cx);
    let adjusted = cx
        .try_global::<GitCommitBufferFontSize>()
        .map_or(size, |s| s.0);
    cx.set_global(GitCommitBufferFontSize(clamp_font_size(f(adjusted))));
    cx.refresh_windows();
}

pub fn reset_git_commit_buffer_font_size(cx: &mut App) {
    if cx.has_global::<GitCommitBufferFontSize>() {
        cx.remove_global::<GitCommitBufferFontSize>();
        cx.refresh_windows();
    }
}

pub fn adjust_markdown_preview_font_size(cx: &mut App, f: impl FnOnce(Pixels) -> Pixels) {
    let size = ThemeSettings::get_global(cx).markdown_preview_font_size(cx);
    let adjusted = cx
        .try_global::<MarkdownPreviewFontSize>()
        .map_or(size, |s| s.0);
    cx.set_global(MarkdownPreviewFontSize(clamp_font_size(f(adjusted))));
    cx.refresh_windows();
}

pub fn reset_markdown_preview_font_size(cx: &mut App) {
    if cx.has_global::<MarkdownPreviewFontSize>() {
        cx.remove_global::<MarkdownPreviewFontSize>();
        cx.refresh_windows();
    }
}

/// 对齐 zed `theme_settings::setup_ui_font` —— 把 `ui_font_size` 同步到窗口 rem size。
pub fn setup_ui_font(window: &mut Window, cx: &mut App) -> Font {
    let (ui_font, ui_font_size) = {
        let theme_settings = ThemeSettings::get_global(cx);
        let font = theme_settings.ui_font.clone();
        (font, theme_settings.ui_font_size(cx))
    };

    window.set_rem_size(ui_font_size);
    ui_font
}

// ---------- set_mode ----------

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

// ---------- buffer_line_height_from_settings ----------

pub fn buffer_line_height_from_settings(
    value: settings::BufferLineHeight,
) -> BufferLineHeight {
    match value {
        settings::BufferLineHeight::Comfortable => BufferLineHeight::Comfortable,
        settings::BufferLineHeight::Standard => BufferLineHeight::Standard,
        settings::BufferLineHeight::Custom(line_height) => BufferLineHeight::Custom(line_height),
    }
}

// ---------- settings::Settings impl ----------

impl settings::Settings for ThemeSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let t = content.theme.as_ref();
        let markdown_preview = content.markdown_preview.as_ref();

        Self {
            ui_font_size: clamp_font_size(
                t.ui_font_size
                    .map(|s| s.into_gpui())
                    .unwrap_or_else(|| px(16.0)),
            ),
            ui_font: Font {
                family: t
                    .ui_font_family
                    .as_ref()
                    .map(|f| f.0.clone().into())
                    .unwrap_or_else(|| ".SystemUIFont".into()),
                weight: t
                    .ui_font_weight
                    .map(|w| w.into_gpui())
                    .unwrap_or(gpui::FontWeight(400.0)),
                style: Default::default(),
                features: t
                    .ui_font_features
                    .clone()
                    .map(|f| f.into_gpui())
                    .unwrap_or_default(),
                fallbacks: None,
            },
            buffer_font_size: clamp_font_size(
                t.buffer_font_size
                    .map(|s| s.into_gpui())
                    .unwrap_or_else(|| px(15.0)),
            ),
            buffer_font: Font {
                family: t
                    .buffer_font_family
                    .as_ref()
                    .map(|f| f.0.clone().into())
                    .unwrap_or_else(|| ".SystemUIFont".into()),
                weight: t
                    .buffer_font_weight
                    .map(|w| w.into_gpui())
                    .unwrap_or(gpui::FontWeight(400.0)),
                style: Default::default(),
                features: t
                    .buffer_font_features
                    .clone()
                    .map(|f| f.into_gpui())
                    .unwrap_or_default(),
                fallbacks: None,
            },
            agent_ui_font_family: t
                .agent_ui_font_family
                .as_ref()
                .map(|f| f.0.clone().into()),
            agent_ui_font_size: t.agent_ui_font_size.map(|s| s.into_gpui()),
            agent_buffer_font_family: t
                .agent_buffer_font_family
                .as_ref()
                .map(|f| f.0.clone().into()),
            agent_buffer_font_size: t.agent_buffer_font_size.map(|s| s.into_gpui()),
            git_commit_buffer_font_size: t.git_commit_buffer_font_size.map(|s| s.into_gpui()),
            markdown_preview_font_family: markdown_preview
                .and_then(|p| p.font_family.as_ref())
                .map(|f| f.0.clone().into()),
            markdown_preview_code_font_family: markdown_preview
                .and_then(|p| p.code_font_family.as_ref())
                .map(|f| f.0.clone().into()),
            markdown_preview_font_size: markdown_preview
                .and_then(|p| p.font_size)
                .map(|s| s.into_gpui()),
            markdown_preview_theme: markdown_preview
                .and_then(|p| p.theme.clone())
                .map(ThemeSelection::from),
            buffer_line_height: buffer_line_height_from_settings(
                t.buffer_line_height
                    .clone()
                    .unwrap_or(settings::BufferLineHeight::Standard),
            ),
            theme: t
                .theme
                .clone()
                .map(ThemeSelection::from)
                .unwrap_or_default(),
            icon_theme: t
                .icon_theme
                .clone()
                .map(IconThemeSelection::from)
                .unwrap_or_default(),
            experimental_theme_overrides: t.experimental_theme_overrides.clone(),
            theme_overrides: t.theme_overrides.clone(),
            ui_density: t.ui_density.unwrap_or_default(),
            unnecessary_code_fade: t
                .unnecessary_code_fade
                .map(|c| c.0.clamp(0.0, 0.9))
                .unwrap_or(0.5),
        }
    }
}

// ---------- appearance_to_mode ----------

pub fn appearance_to_mode(appearance: Appearance) -> ThemeAppearanceMode {
    match appearance {
        Appearance::Light => ThemeAppearanceMode::Light,
        Appearance::Dark => ThemeAppearanceMode::Dark,
    }
}
