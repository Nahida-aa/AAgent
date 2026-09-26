//! ThemeSettings — UI + theme 相关设置 + 主题系统初始化。
//!
//! 对齐 Zed 分层：
//! - `aa_gpui_kit_theme` = Zed 的 `crates/theme`（核心类型、注册表、set_theme）
//! - `theme-settings`    = Zed 的 `crates/theme_settings`（装配、设置集成）

pub mod settings;

pub use settings::ThemeSettings;
// 注意: crate 内部有同名 `pub mod settings` 遮蔽了外部 settings crate，
// 所以用 ::settings::Settings 绝对路径，或直接 extern crate 改名。
#[allow(unused_imports)]
use ::settings::Settings;

use std::borrow::Cow;
use std::sync::Arc;

use aa_gpui_kit_theme::registry::ThemeRegistry;
use aa_gpui_kit_theme::{GlobalTheme, default_colors::catppuccin_mocha, set_theme};
use aa_gpui_kit_theme::{IconTheme, LoadThemes, Theme};
use gpui::{App, AssetSource, Font, Result, SharedString};

/// 把 gpui 全局里的 `Arc<dyn AssetSource>` 适配成注册表要的 `Box<dyn AssetSource>`。
/// Zed 的做法是 init 时由调用方把资产传进来（`LoadThemes::All(assets)`），
/// 我们保持「资产在 gpui 全局」的现有约定，用适配器桥接。
struct GlobalAssets(Arc<dyn AssetSource>);

impl AssetSource for GlobalAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> { self.0.load(path) }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> { self.0.list(path) }
}

/// 安装主题系统（应用启动时调用一次）。
/// 对齐 Zed `theme_settings::init` (crates/theme_settings/src/theme_settings.rs L71) —
/// Zed 先调 `theme::init` 做基础装配，再装 settings provider，再 observe settings 变化。
/// 这里先实现 `theme::init` 那部分（gpui_learn 的 init_theme 逻辑简化版）。
pub fn init(themes_to_load: LoadThemes, cx: &mut App) {
    // 1. 用 app 的 asset_source 构造注册表（gpui_learn ThemeRegistry::new 自带 Catppuccin 内置主题）
    let assets: Box<dyn AssetSource> = Box::new(GlobalAssets(cx.asset_source().clone()));
    ThemeRegistry::set_global(assets, cx);

    // 2. 选默认主题（优先注册表的 "Catppuccin Mocha"，拿不到就用内置构造）
    let registry = ThemeRegistry::global(cx);
    let theme = registry
        .get("Catppuccin Mocha")
        .map(|t| (*t).clone())
        .unwrap_or_else(|_| catppuccin_mocha());
    set_theme(cx, Arc::new(theme));
}

/// 把当前主题按 ThemeSettings + SystemAppearance 重新解析并应用。
/// 对齐 Zed `theme_settings::reload_theme` (crates/theme_settings/src/theme_settings.rs L204)。
pub fn reload_theme(cx: &mut App) {
    let theme = configured_theme(cx);
    let global = cx.global::<GlobalTheme>();
    let icon_theme = global.icon_theme.clone();
    cx.set_global(GlobalTheme::new(theme, icon_theme));
}

/// 把当前图标主题按 ThemeSettings + SystemAppearance 重新解析并应用。
/// 对齐 Zed `theme_settings::reload_icon_theme` (crates/theme_settings/src/theme_settings.rs L211)。
pub fn reload_icon_theme(cx: &mut App) {
    let icon_theme = configured_icon_theme(cx);
    let global = cx.global::<GlobalTheme>();
    let theme = global.theme.clone();
    cx.set_global(GlobalTheme::new(theme, icon_theme));
}

/// 读取 ThemeSettings.ui_font 并返回给调用方设置窗口 rem size。
/// 对齐 Zed `theme_settings::setup_ui_font` (crates/theme_settings/src/settings.rs L587)。
/// Zed 会调 `window.set_rem_size(ui_font_size)`，但我们的 gpui 版本还没有这个 API，
/// 所以这里只返回 Font，让调用方后续扩展。
pub fn setup_ui_font(cx: &mut App) -> Font {
    let theme_settings = ThemeSettings::get_global(cx);
    theme_settings.ui_font.clone()
}

fn configured_theme(cx: &mut App) -> Arc<Theme> {
    let registry = ThemeRegistry::default_global(cx);
    let theme_settings = ThemeSettings::get_global(cx);

    // 我们 ThemeSettings.theme 是 serde_json::Value（简化版），Zed 是 ThemeSelection enum。
    // 从 Value 里拿 .name 字段，拿不到就用默认。
    let theme_name = theme_settings
        .theme
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Catppuccin Mocha".to_string());

    match registry.get(&theme_name) {
        Ok(theme) => theme,
        Err(_) => {
            let fallback = registry
                .get("Catppuccin Mocha")
                .unwrap_or_else(|_| Arc::new(catppuccin_mocha()));
            fallback
        }
    }
}

fn configured_icon_theme(cx: &mut App) -> Arc<IconTheme> {
    let registry = ThemeRegistry::default_global(cx);
    let theme_settings = ThemeSettings::get_global(cx);

    // icon_theme 是 Option<String>，直接用或回退默认
    match &theme_settings.icon_theme {
        Some(name) => registry
            .get_icon_theme(name)
            .unwrap_or_else(|_| registry.default_icon_theme().unwrap_or_else(|_| Arc::new(IconTheme::default()))),
        None => registry.default_icon_theme().unwrap_or_else(|_| Arc::new(IconTheme::default())),
    }
}
