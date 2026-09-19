//! ThemeSettings — UI + theme 相关设置 + 主题系统初始化。
//!
//! 对齐 Zed 分层：
//! - `aa_gpui_kit_theme` = Zed 的 `crates/theme`（核心类型、注册表、set_theme）
//! - `theme-settings`    = Zed 的 `crates/theme_settings`（装配、设置集成）

pub mod settings;

use std::borrow::Cow;
use std::sync::Arc;

use aa_gpui_kit_theme::default_colors::catppuccin_mocha;
use aa_gpui_kit_theme::registry::ThemeRegistry;
use aa_gpui_kit_theme::set_theme;
use gpui::{App, AssetSource, Result, SharedString};

/// 把 gpui 全局里的 `Arc<dyn AssetSource>` 适配成注册表要的 `Box<dyn AssetSource>`。
/// Zed 的做法是 init 时由调用方把资产传进来（`LoadThemes::All(assets)`），
/// 我们保持「资产在 gpui 全局」的现有约定，用适配器桥接。
struct GlobalAssets(Arc<dyn AssetSource>);

impl AssetSource for GlobalAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        self.0.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        self.0.list(path)
    }
}

/// 安装主题系统（应用启动时调用一次）。
/// 对齐 Zed `theme_settings::init` (crates/theme_settings/src/theme_settings.rs L71) —
/// Zed 先调 `theme::init` 做基础装配，再装 settings provider，再 observe settings 变化。
/// 这里先实现 `theme::init` 那部分（gpui_learn 的 init_theme 逻辑简化版）。
pub fn init_theme(cx: &mut App) {
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
