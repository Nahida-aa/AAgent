//! 状态栏内置 item：与 zed `workspace::status_bar` + `crates/zed/src/zed.rs:633`
//! 的装配对齐。
//!
//! 左右分组与渲染方式见 `status_bar` 模块头。这里每个 struct 对应 zed 的
//! 一个状态栏项；**完全模拟 zed 的条件显示**：无数据时渲染空元素
//! （`gpui::Empty`），与 zed 空闲时的表现一致。后续接数据管线时把相应
//! 项从占位换成真实现即可。
//!
//! 装配顺序（`add_left_item` / `add_right_item` / `add_panel`，右组渲染反序）：
//!
//! | 注册序 | zed item                    | 本实现                      | 现状            |
//! |--------|-----------------------------|-----------------------------|-----------------|
//! | 面板   | left dock 面板按钮          | `PanelKind`（Project/Git，由 StatusBar 渲染） | 可见（占位） |
//! | 面板   | right dock 面板按钮        | `PanelKind`（Collab/Outline/Terminal/Debug/Agent，由 StatusBar 渲染） | 可见（占位） |
//! | 左 1   | search                      | `Search`（放大镜）          | 可见（占位）    |
//! | 左 2   | lsp_button                  | `LanguageServers`（闪电）   | 空（无服务器）  |
//! | 左 3   | diagnostic_indicator        | `Diagnostics`（对勾）       | 可见（占位）    |
//! | 左 4   | active_file_name            | `ActiveFileName`            | 空（无 buffer） |
//! | 左 5   | git_blame_status            | `GitBlame`                  | 空（无 blame）  |
//! | 左 6   | merge_conflict_indicator    | `MergeConflict`             | 空（无冲突）    |
//! | 左 7   | activity_indicator          | `ActivityIndicator`         | 空（无活动）    |
//! | 右 1   | edit_prediction             | `EditPrediction`            | 可见（占位）    |
//! | 右 2   | active_buffer_encoding      | `Encoding`（UTF-8）         | 可见（占位）    |
//! | 右 3   | active_buffer_language      | `Language`（Rust）          | 可见（占位）    |
//! | 右 4   | active_toolchain            | `Toolchain`                 | 空（无工具链）  |
//! | 右 5   | line_ending                 | `LineEnding`（LF）          | 可见（占位）    |
//! | 右 6   | cursor_position             | `CursorPosition`（1:1）     | 可见（占位）    |
//! | 右 7   | image_info                  | `ImageInfo`                 | 空（无图片）    |
//! | 右 8   | vim_mode_indicator          | `VimMode`                   | 空（无 vim）    |
//! | 右 9   | pending_keystrokes          | `PendingKeystrokes`         | 空（无按键）    |
//! | 尾     | threads sidebar toggle      | `StatusBar::render_right_tools` | 可见（占位）|
//!
//! 右键行为（对齐 zed，内容因按钮而异）：
//! - dock 面板按钮（左/右组两端）：菜单 = `Dock Left / Dock Right / Dock Bottom`，
//!   当前 dock 位置勾选，点击把该按钮搬到对应组（`StatusBar::render_panel`，
//!   见 [`super::StatusBar`]）。
//! - 普通项（Search/Diagnostics/Encoding 等可见项）：菜单 = `Hide`，点击隐藏该项
//!   （`StatusBar::render_hideable`）。

use aa_gpui_kit_theme::{ActiveTheme, ThemeColors};
use aa_gpui_kit_ui::{ButtonRadius, Icon, IconButton, IconName};
use gpui::{
    Context, Div, ElementId, Empty, ParentElement, Render, SharedString, Stateful, Styled, Window,
    div, prelude::*, px,
};

use super::StatusItemView;

/// 状态栏文字按钮：复刻 zed `Button + LabelSize::Small + ButtonStyle::Subtle`。
/// 22px 高、12px 文字、透明底、hover 出 ghost 背景、圆角 md、指针手型。
pub struct StatusButton {
    id: ElementId,
    label: SharedString,
    aria_label: Option<SharedString>,
    colors: ThemeColors,
}

impl StatusButton {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        colors: ThemeColors,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            aria_label: None,
            colors,
        }
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl IntoElement for StatusButton {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        let mut button = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .h(px(22.0))
            .px_1()
            .rounded_md()
            .text_size(px(12.0))
            .text_color(self.colors.text)
            .cursor_pointer()
            .hover(move |style| style.bg(self.colors.ghost_element_hover))
            .child(self.label);

        if let Some(aria_label) = self.aria_label {
            button = button.aria_label(aria_label);
        }

        button
    }
}

/// 左 1：搜索（≈ zed search_button）。放大镜图标按钮，暂无行为。
pub struct Search;

impl Search {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for Search {}

impl Render for Search {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        IconButton::new("status-search", IconName::MagnifyingGlass)
            .size(px(22.0))
            .icon_size(px(14.0))
            .radius(ButtonRadius::Medium)
            .aria_label("Project Search")
    }
}

/// 左 2：语言服务器（≈ zed lsp_button）。无服务器时隐藏。
pub struct LanguageServers;

impl LanguageServers {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for LanguageServers {}

impl Render for LanguageServers {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 左 3：诊断摘要（≈ zed DiagnosticIndicator）。无诊断时显示灰色对勾。
pub struct Diagnostics;

impl Diagnostics {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for Diagnostics {}

impl Render for Diagnostics {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        Icon::new(IconName::Check).size(px(14.0)).color(colors.text)
    }
}

/// 左 4：当前文件名（≈ zed active_file_name）。
///
/// 显示活动缓冲区的文件名（如 `main.rs`），不是项目名（项目名在标题栏）。
/// 无缓冲区时隐藏，与 zed 空闲一致。
pub struct ActiveFileName {
    name: Option<SharedString>,
}

impl ActiveFileName {
    /// 有活动缓冲区时调用，传入文件名。
    pub fn new(name: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Self {
            name: Some(name.into()),
        }
    }

    /// 无缓冲区（隐藏）。
    pub fn empty(_cx: &mut Context<Self>) -> Self {
        Self { name: None }
    }
}

impl StatusItemView for ActiveFileName {}

impl Render for ActiveFileName {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.name {
            Some(name) => StatusButton::new(
                "status-active-file-name",
                name.clone(),
                cx.theme().colors().clone(),
            )
            .into_any_element(),
            None => Empty.into_any_element(),
        }
    }
}

/// 左 5：git blame（≈ zed git_blame_status）。无 blame 时隐藏。
pub struct GitBlame;

impl GitBlame {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for GitBlame {}

impl Render for GitBlame {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 左 6：合并冲突（≈ zed merge_conflict_indicator）。无冲突时隐藏。
pub struct MergeConflict;

impl MergeConflict {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for MergeConflict {}

impl Render for MergeConflict {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 左 7：活动指示器（≈ zed activity_indicator）。无活动时隐藏。
pub struct ActivityIndicator;

impl ActivityIndicator {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for ActivityIndicator {}

impl Render for ActivityIndicator {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 右 1：智能补全（≈ zed edit_prediction）。ZedPredict 图标 + 状态点。
pub struct EditPrediction;

impl EditPrediction {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for EditPrediction {}

impl Render for EditPrediction {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        gpui::div()
            .relative()
            .child(
                IconButton::new("status-edit-prediction", IconName::ZedPredict)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ButtonRadius::Square)
                    .aria_label("Edit Prediction"),
            )
            .child(
                gpui::div()
                    .absolute()
                    .right_1()
                    .bottom_1()
                    .size_1p5()
                    .rounded_full()
                    .bg(colors.text)
                    .border_1()
                    .border_color(colors.text_accent),
            )
    }
}

/// 右 2：缓冲区编码（≈ zed active_buffer_encoding）。占位 UTF-8。
pub struct Encoding;

impl Encoding {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for Encoding {}

impl Render for Encoding {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        StatusButton::new("status-encoding", "UTF-8", cx.theme().colors().clone())
    }
}

/// 右 3：缓冲区语言（≈ zed active_buffer_language）。占位 Rust。
pub struct Language;

impl Language {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for Language {}

impl Render for Language {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        StatusButton::new("status-language", "Rust", cx.theme().colors().clone())
            .aria_label("Language: Rust")
    }
}

/// 右 4：工具链（≈ zed active_toolchain）。无工具链时隐藏。
pub struct Toolchain;

impl Toolchain {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for Toolchain {}

impl Render for Toolchain {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 右 5：行尾（≈ zed line_ending_flag）。占位 LF。
pub struct LineEnding;

impl LineEnding {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for LineEnding {}

impl Render for LineEnding {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        StatusButton::new("status-line-ending", "LF", cx.theme().colors().clone())
    }
}

/// 右 6：光标位置（≈ zed cursor_position）。占位 1:1。
pub struct CursorPosition;

impl CursorPosition {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for CursorPosition {}

impl Render for CursorPosition {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        StatusButton::new("status-cursor-position", "1:1", cx.theme().colors().clone())
            .aria_label("Line 1, column 1")
    }
}

/// 右 7：图片信息（≈ zed image_info）。无图片时隐藏。
pub struct ImageInfo;

impl ImageInfo {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for ImageInfo {}

impl Render for ImageInfo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 右 8：vim 模式指示（≈ zed vim_mode_indicator）。无 vim 时隐藏。
pub struct VimMode;

impl VimMode {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for VimMode {}

impl Render for VimMode {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 右 9：待决按键（≈ zed pending_keystrokes）。无按键时隐藏。
pub struct PendingKeystrokes;

impl PendingKeystrokes {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl StatusItemView for PendingKeystrokes {}

impl Render for PendingKeystrokes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

// PanelKind 已搬到 crate::panel（与 Dock/DockPosition 同层）。
// 此文件只保留 StatusBar 普通状态项的 Render 实现。
