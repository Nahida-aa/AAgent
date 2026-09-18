//! Item 相关的 settings 类型。
//!
//! 对齐 Zed `ItemSettings` + `PreviewTabsSettings`。
//! Zed 用 `#[derive(RegisterSetting)]` 注册到 settings store。
//! AAgent 还没 settings 系统，先放纯数据结构占位。

/// Item 级别的 settings — 控制 tab bar 外观和行为。
///
/// 对齐 Zed `ItemSettings`（settings_content.rs 里的 tabs 段）。
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone, Default)]
pub struct PreviewTabsSettings {
    pub enabled: bool,
    pub enable_preview_from_project_panel: bool,
    pub enable_preview_from_file_finder: bool,
}

/// 关闭 tab 时的位置偏好。对齐 Zed `ClosePosition`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClosePosition {
    /// 关闭按钮在 tab 右侧（大多数编辑器默认）。
    #[default]
    Right,
    Left,
}

/// 关闭 active tab 后激活哪个。对齐 Zed `ActivateOnClose`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActivateOnClose {
    /// 激活最近激活过的 tab — Zed 默认。
    #[default]
    MostRecentlyActivated,
    Left,
    Right,
}

/// tab 上 diagnostics 显示策略。对齐 Zed `ShowDiagnostics`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShowDiagnostics {
    #[default]
    All,
    ErrorsOnly,
    None,
}

/// tab 关闭按钮显示策略。对齐 Zed `ShowCloseButton`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShowCloseButton {
    /// 只有 hover 时显示 — Zed 默认。
    #[default]
    OnHover,
    Always,
    Never,
}
