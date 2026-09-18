//! MultiWorkspace — 顶层窗口容器（对齐 zed `MultiWorkspace`）。
//!
//! Zed 的 MultiWorkspace 职责（multi_workspace.rs:L304-L322）：
//! - 管理多个 Workspace（窗口分屏）
//! - 持有 Sidebar（Agent Threads 列表 + Recent Projects）
//! - Sidebar 独立于 Dock — 渲染顺序: [sidebar?] Workspace(main_area) [sidebar?]
//!
//! AAgent 现阶段：
//! - 单 Workspace（不需要多窗口）
//! - Sidebar 是占位容器（200px 宽，等后续实现真正的 Threads 列表）
//! - Sidebar 状态由 StatusBar 持有（toggle_sidebar 改 StatusBar 的 SidebarStatus）

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use crate::Workspace;
use crate::dock_position::DockPosition;
use crate::status_bar::SidebarStatus;

/// 顶层 MultiWorkspace entity。
pub struct MultiWorkspace {
    workspace: Entity<Workspace>,
}

impl MultiWorkspace {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        // 订阅 workspace — StatusBar sidebar toggle 改变状态时触发 MultiWorkspace 重渲染
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();

        Self { workspace }
    }

    pub fn workspace(&self) -> &Entity<Workspace> {
        &self.workspace
    }

    /// 读取 sidebar 状态 — 来自 StatusBar（Zed 里是 MultiWorkspace 自己持有）。
    /// AAgent 简化：StatusBar 负责 sidebar toggle，MultiWorkspace 负责渲染。
    fn sidebar_state(&self, cx: &App) -> SidebarStatus {
        self.workspace.read(cx).status_bar().read(cx).sidebar()
    }
}

impl Render for MultiWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let sidebar = self.sidebar_state(cx);
        let has_left_sidebar = sidebar.open && sidebar.side == DockPosition::Left;
        let has_right_sidebar = sidebar.open && sidebar.side == DockPosition::Right;

        // Zed MultiWorkspace 渲染顺序: [sidebar?] Workspace [sidebar?]
        // Sidebar 独立于 Dock（属于 MultiWorkspace 层，crates/sidebar/src/sidebar.rs）
        let render_sidebar = |side: DockPosition| {
            let id = match side {
                DockPosition::Left => "workspace-sidebar-left",
                DockPosition::Right => "workspace-sidebar-right",
                DockPosition::Bottom => unreachable!(),
            };
            div()
                .id(id)
                .flex_none()
                .w(px(200.))
                .h_full()
                .bg(colors.surface_background)
                .border_r_1()
                .border_color(colors.border_variant)
        };

        div()
            .id("multi-workspace")
            .flex()
            .flex_row()
            .size_full()
            .bg(colors.panel_background)
            .when(has_left_sidebar, |el| {
                el.child(render_sidebar(DockPosition::Left))
            })
            .child(self.workspace.clone())
            .when(has_right_sidebar, |el| {
                el.child(render_sidebar(DockPosition::Right))
            })
    }
}
