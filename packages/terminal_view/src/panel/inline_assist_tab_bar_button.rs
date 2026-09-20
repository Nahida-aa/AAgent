//! InlineAssistTabBarButton — 终端 tab bar 上的 Inline Assist 按钮。
//!
//! 对齐 Zed `terminal_panel.rs` L1854-1873 的同名 struct。
//! Zed 版是 Pane tab bar 上的一个 IconButton，点了发 `InlineAssist` action。
//!
//! 我们目前没有 InlineAssist action，这个组件先占好位置 —
//! terminal tab bar 上的 "assist" 按钮，未来接 Agent/InlineAssist 功能。

use gpui::{App, FocusHandle, IntoElement, RenderOnce, Window};

/// Pane tab bar 上的 Inline Assist 按钮。
///
/// 构造时传入 terminal view 的 focus_handle，点按钮时 dispatch 到这个 handle。
/// Zed 用 `InlineAssist::default()` action，我们简化为 stub（没有 Agent action）。
pub struct InlineAssistTabBarButton {
    #[allow(dead_code)]
    focus_handle: FocusHandle,
}

impl InlineAssistTabBarButton {
    pub fn new(focus_handle: FocusHandle) -> Self { Self { focus_handle } }
}

impl RenderOnce for InlineAssistTabBarButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Zed 版用 IconButton + IconName::ZedAssistant + InlineAssist action
        // 我们没有 InlineAssist action，也没有 ZedAssistant 图标，
        // 先返回空 element，等 Agent 功能接入再接
        gpui::Empty
    }
}
