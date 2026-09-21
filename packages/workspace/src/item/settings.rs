//! Item 相关的 settings 类型。
//!
//! 对齐 Zed `ItemSettings` + `PreviewTabsSettings`。
//! Zed 用 `#[derive(RegisterSetting)]` 注册到 settings store。
//! AAgent 还没 settings 系统，先放纯数据结构占位。

use settings::{
    ActivateOnClose, ClosePosition, RegisterSetting, Settings, ShowCloseButton, ShowDiagnostics,
};

/// Item 级别的 settings — 控制 tab bar 外观和行为。
///
/// 对齐 Zed `ItemSettings`（settings_content.rs 里的 tabs 段）。
#[derive(RegisterSetting)]
pub struct ItemSettings {
    /// tab 上是否显示 git status icon（新增/修改/未追踪）。
    pub git_status: bool,
    /// 关闭 tab 时，激活左边还是右边的 tab。
    pub close_position: ClosePosition,
    /// 关闭 active tab 后激活哪个（最近激活 / 左 / 右）。
    pub activate_on_close: ActivateOnClose,
    /// tab icon 是否显示文件类型图标（比 icon 更细粒度）。
    pub file_icons: bool,
    /// tab 上是否显示 diagnostics 数量 badge。
    pub show_diagnostics: ShowDiagnostics,
    /// tab 上是否显示关闭按钮。
    pub show_close_button: ShowCloseButton,
}

/// Preview tab 相关 settings。
///
/// 对齐 Zed `PreviewTabsSettings`。
/// Preview tab = 点一下文件先在 tab bar 临时打开，再点别的就关掉
/// （VSCode / Zed 都有这个功能）。
#[derive(RegisterSetting)]
pub struct PreviewTabsSettings {
    pub enabled: bool,
    pub enable_preview_from_project_panel: bool,
    pub enable_preview_from_file_finder: bool,
    pub enable_preview_from_multibuffer: bool,
    pub enable_preview_multibuffer_from_code_navigation: bool,
    pub enable_preview_file_from_code_navigation: bool,
    pub enable_keep_preview_on_code_navigation: bool,
}

impl Settings for ItemSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let tabs = content.tabs.as_ref().unwrap();
        Self {
            git_status: tabs.git_status.unwrap()
                && content
                    .git
                    .as_ref()
                    .unwrap()
                    .enabled
                    .unwrap()
                    .is_git_status_enabled(),
            close_position: tabs.close_position.unwrap(),
            activate_on_close: tabs.activate_on_close.unwrap(),
            file_icons: tabs.file_icons.unwrap(),
            show_diagnostics: tabs.show_diagnostics.unwrap(),
            show_close_button: tabs.show_close_button.unwrap(),
        }
    }
}

impl Settings for PreviewTabsSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let preview_tabs = content.preview_tabs.as_ref().unwrap();
        Self {
            enabled: preview_tabs.enabled.unwrap(),
            enable_preview_from_project_panel: preview_tabs
                .enable_preview_from_project_panel
                .unwrap(),
            enable_preview_from_file_finder: preview_tabs.enable_preview_from_file_finder.unwrap(),
            enable_preview_from_multibuffer: preview_tabs.enable_preview_from_multibuffer.unwrap(),
            enable_preview_multibuffer_from_code_navigation: preview_tabs
                .enable_preview_multibuffer_from_code_navigation
                .unwrap(),
            enable_preview_file_from_code_navigation: preview_tabs
                .enable_preview_file_from_code_navigation
                .unwrap(),
            enable_keep_preview_on_code_navigation: preview_tabs
                .enable_keep_preview_on_code_navigation
                .unwrap(),
        }
    }
}
