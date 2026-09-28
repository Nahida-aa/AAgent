//! ThemeSettings — UI + theme 相关设置 + 主题系统初始化。
//!
//! 对齐 Zed 分层：
//! - `theme` = Zed 的 `crates/theme`（核心类型、注册表、set_theme）
//! - `theme-settings`    = Zed 的 `crates/theme_settings`（装配、设置集成）

mod schema;
pub mod settings;

pub use schema::{
    ThemeContent, ThemeFamilyContent, status_colors_refinement, syntax_overrides,
    theme_colors_refinement,
};
pub use settings::{
    AgentBufferFontSize, AgentUiFontSize, BufferFontSize,
    GitCommitBufferFontSize, IconThemeSelection, MarkdownPreviewFontSize, UiFontSize,
    ThemeAppearanceMode, ThemeSelection, ThemeSettings,
    adjust_agent_buffer_font_size, adjust_agent_ui_font_size, adjust_buffer_font_size,
    adjust_git_commit_buffer_font_size, adjust_markdown_preview_font_size, adjust_ui_font_size,
    appearance_to_mode, adjusted_font_size, buffer_line_height_from_settings, clamp_font_size,
    decrease_buffer_font_size, increase_buffer_font_size, observe_buffer_font_size_adjustment,
    reset_agent_buffer_font_size, reset_agent_ui_font_size, reset_buffer_font_size,
    reset_git_commit_buffer_font_size, reset_markdown_preview_font_size, reset_ui_font_size,
    set_mode,
};

// setup_ui_font 在 lib.rs 里也有定义（下面），不从 settings.rs re-export。

// BufferLineHeight — theme 版本（有 .value() 方法）。
// 用完整路径 re-export 避免 settings crate 里的同名冲突。
pub use theme::BufferLineHeight;

// ThemeStyleContent / HighlightStyleContent / ThemeName / IconThemeName
// 由 settings_content crate 定义（settings crate 也 re-export 了）。
// 注意：crate 内部的 `pub mod settings` 遮蔽了外部 settings crate，
// 所以要用 `::settings::` 绝对路径或直接用 `settings_content::`。
// Zed 原版 theme_settings re-export 它们，保持同样 import 路径。
pub use ::settings::{HighlightStyleContent, IconThemeName, ThemeName, ThemeStyleContent};

// 注意: crate 内部有同名 `pub mod settings` 遮蔽了外部 settings crate，
// 所以用 ::settings::Settings 绝对路径，或直接 extern crate 改名。
#[allow(unused_imports)]
use ::settings::Settings;

use std::sync::Arc;

use theme::ActiveTheme;
use theme::registry::ThemeRegistry;
use theme::{GlobalTheme, SystemAppearance, default_colors::catppuccin_macchiato, set_theme};
use theme::{IconTheme, LoadThemes, Theme};
use gpui::{App, Font, Window};
use ::settings::SettingsStore;

/// 安装主题系统（应用启动时调用一次）。
///
/// 完整对齐 Zed `theme_settings::init` (crates/theme_settings/src/theme_settings.rs L71)：
/// 1. `theme::init` 做基础装配（SystemAppearance + ThemeRegistry）
/// 2. 从 SettingsStore 解析初始主题 + 图标主题
/// 3. 应用主题 —— **必须用 `theme::set_theme()`**，它内部会调 `sync_global_colors`
///    把主题语义色同步到 gpui 的 8 色兜底（gpui::GlobalColors），gpui 内置元素
///    靠这个渲染。如果直接 cx.set_global(GlobalTheme::new) 就会跳过 sync，
///    导致 StatusBar / TitleBar / dock 等区域颜色不对。
/// 4. `observe_global::<SettingsStore>` 监听变化自动 reload
///
/// **前置条件**: 必须在 `settings::init(cx)` 之后调用，这样 SettingsStore 已存在。
pub fn init(themes_to_load: LoadThemes, cx: &mut App) {
    // 1. 基础主题系统装配（对齐 Zed `theme::init`）。
    // gpui_learn 的 theme::init 封装了 SystemAppearance::init + ThemeRegistry::set_global。
    theme::init(themes_to_load, cx);

    // 2. 从 SettingsStore 解析初始主题 + 图标主题（SettingsStore 此时应已存在）
    let theme = configured_theme(cx);
    let icon_theme = configured_icon_theme(cx);

    // 3. 应用主题 —— 先用 set_theme 触发 sync_global_colors，再覆盖 icon_theme。
    // set_theme 内部会：
    //   (a) sync_global_colors(cx, &theme)  ← 关键！gpui::GlobalColors 8 色兜底
    //   (b) cx.set_global(GlobalTheme::new(theme, default_icon_theme))
    // 然后我们再 set_global 一次，覆盖 icon_theme 为 configured 的版本。
    set_theme(cx, theme.clone());
    cx.set_global(GlobalTheme::new(theme, icon_theme));

    // 4. observe SettingsStore 变化自动 reload（对齐 Zed L104-L161）
    let settings = ThemeSettings::get_global(cx);
    let mut prev_theme_name = settings.theme.name(SystemAppearance::global(cx).0);
    let mut prev_icon_theme_name = settings.icon_theme.name(SystemAppearance::global(cx).0);
    let mut prev_theme_overrides = (
        settings.experimental_theme_overrides.clone(),
        settings.theme_overrides.clone(),
    );

    cx.observe_global::<SettingsStore>(move |cx| {
        let settings = ThemeSettings::get_global(cx);
        let theme_name = settings.theme.name(SystemAppearance::global(cx).0);
        let icon_theme_name = settings.icon_theme.name(SystemAppearance::global(cx).0);
        let theme_overrides = (
            settings.experimental_theme_overrides.clone(),
            settings.theme_overrides.clone(),
        );

        if theme_name != prev_theme_name || theme_overrides != prev_theme_overrides {
            prev_theme_name = theme_name;
            prev_theme_overrides = theme_overrides;
            reload_theme(cx);
        }

        if icon_theme_name != prev_icon_theme_name {
            prev_icon_theme_name = icon_theme_name;
            reload_icon_theme(cx);
        }
    })
    .detach();
}

/// 把当前主题按 ThemeSettings + SystemAppearance 重新解析并应用。
/// 对齐 Zed `theme_settings::reload_theme` (crates/theme_settings/src/theme_settings.rs L204)。
pub fn reload_theme(cx: &mut App) {
    let theme = configured_theme(cx);
    // 先 set_theme 触发 sync_global_colors，再覆盖 icon_theme
    set_theme(cx, theme.clone());
    let global = cx.global::<GlobalTheme>();
    let icon_theme = global.icon_theme.clone();
    cx.set_global(GlobalTheme::new(theme, icon_theme));
    cx.refresh_windows();
}

/// 把当前图标主题按 ThemeSettings + SystemAppearance 重新解析并应用。
/// 对齐 Zed `theme_settings::reload_icon_theme` (crates/theme_settings/src/theme_settings.rs L211)。
pub fn reload_icon_theme(cx: &mut App) {
    let icon_theme = configured_icon_theme(cx);
    let global = cx.global::<GlobalTheme>();
    let theme = global.theme.clone();
    cx.set_global(GlobalTheme::new(theme, icon_theme));
    cx.refresh_windows();
}

/// 读取 ThemeSettings.ui_font，顺手把窗口 rem size 设成 UI 字号。
/// 对齐 Zed `theme_settings::setup_ui_font` (crates/theme_settings/src/settings.rs L587)。
pub fn setup_ui_font(window: &mut Window, cx: &mut App) -> Font {
    let (ui_font, ui_font_size) = {
        let theme_settings = ThemeSettings::get_global(cx);
        (theme_settings.ui_font.clone(), theme_settings.ui_font_size(cx))
    };

    window.set_rem_size(ui_font_size);
    ui_font
}

fn configured_theme(cx: &mut App) -> Arc<Theme> {
    let registry = ThemeRegistry::default_global(cx);
    let theme_settings = ThemeSettings::get_global(cx);

    // 主题名按当前系统明暗解析（对齐 zed：`ThemeSelection::name(system_appearance)`）。
    // 用 SystemAppearance::global 而非 cx.theme().appearance()，
    // 因为 theme_settings::init 时 GlobalTheme 可能还不存在。
    let theme_name = theme_settings.theme.name(SystemAppearance::global(cx).0);

    match registry.get(theme_name.0.as_ref()) {
        Ok(theme) => theme,
        Err(_) => {
            // Fallback 链：Macchiato → Mocha → 硬编码构造。
            // 注意主题名要和 gpui_learn default_colors.rs 里的 name 字段一致。
            let fallback = registry
                .get("Catppuccin Macchiato")
                .or_else(|_| registry.get("Catppuccin Mocha"))
                .or_else(|_| registry.get("ui-gpui Dark"))
                .unwrap_or_else(|_| Arc::new(catppuccin_macchiato()));
            fallback
        }
    }
}

fn configured_icon_theme(cx: &mut App) -> Arc<IconTheme> {
    let registry = ThemeRegistry::default_global(cx);
    let theme_settings = ThemeSettings::get_global(cx);

    // icon_theme 是 IconThemeSelection，按当前系统明暗解析
    let appearance = SystemAppearance::global(cx).0;
    let icon_theme_name = theme_settings.icon_theme.name(appearance);
    registry
        .get_icon_theme(icon_theme_name.0.as_ref())
        .unwrap_or_else(|_| {
            registry
                .default_icon_theme()
                .unwrap_or_else(|_| Arc::new(IconTheme::default()))
        })
}
