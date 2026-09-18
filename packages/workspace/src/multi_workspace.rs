//! MultiWorkspace — 顶层窗口容器（对齐 zed `MultiWorkspace`）。
//!
//! Zed 的 MultiWorkspace 职责（multi_workspace.rs:L304-L322）：
//! - 管理多个 Workspace（窗口分屏）
//! - 持有 Sidebar（Agent Threads 列表 + Recent Projects）
//! - Sidebar 独立于 Dock — 渲染顺序: [sidebar?] Workspace(main_area) [sidebar?]
//!
//! AAgent 现阶段：
//! - 单 Workspace（不需要多窗口）
//! - 单 Sidebar entity（open/close/toggle 由 MultiWorkspace 统一管理）

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use crate::Workspace;
use crate::sidebar::{Sidebar, SidebarStatus};
use settings_content::DockPosition;

/// 顶层 MultiWorkspace entity。
pub struct MultiWorkspace {
    workspace: Entity<Workspace>,
    sidebar: Entity<Sidebar>,
}

impl MultiWorkspace {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(cx));
        // 让 Sidebar 反向引用 MultiWorkspace（close_sidebar 需要）
        let this_entity = cx.entity();
        sidebar.update(cx, |s, cx| {
            s.set_multi_workspace(this_entity.clone());
        });

        // 订阅 workspace — 重渲染
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        // 订阅 sidebar — sidebar open/close/toggle 时重渲染
        cx.observe(&sidebar, |_, _, cx| cx.notify()).detach();

        Self { workspace, sidebar }
    }

    pub fn workspace(&self) -> &Entity<Workspace> {
        &self.workspace
    }

    pub fn sidebar(&self) -> &Entity<Sidebar> {
        &self.sidebar
    }

    pub fn sidebar_status(&self, cx: &App) -> SidebarStatus {
        self.sidebar.read(cx).status()
    }

    /// 对外 API — 切换 sidebar 开关（对齐 zed `ToggleWorkspaceSidebar`）。
    pub fn toggle_sidebar(&mut self, side: DockPosition, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |s, cx| s.toggle(side, cx));
    }

    /// 对外 API — 关闭 sidebar（对齐 zed `CloseWorkspaceSidebar`）。
    pub fn close_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |s, cx| s.close(cx));
    }
}

impl Render for MultiWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let sidebar = self.sidebar.read(cx).status();
        let has_left_sidebar = sidebar.open && sidebar.side == DockPosition::Left;
        let has_right_sidebar = sidebar.open && sidebar.side == DockPosition::Right;

        // Zed MultiWorkspace 渲染顺序: [sidebar?] Workspace [sidebar?]
        // Sidebar 独立于 Dock（属于 MultiWorkspace 层，crates/sidebar/src/sidebar.rs）
        // Sidebar 有自己的 entity — 渲染完整的 Sidebar（thread list + bottom bar）
        let sidebar_entity = self.sidebar.clone();

        div()
            .id("multi-workspace")
            .flex()
            .flex_row()
            .size_full()
            .bg(colors.panel_background)
            .when(has_left_sidebar, |el| {
                el.child(
                    div()
                        .id("workspace-sidebar-left")
                        .flex_none()
                        .w(px(200.))
                        .h_full()
                        .child(sidebar_entity.clone()),
                )
            })
            .child(self.workspace.clone())
            .when(has_right_sidebar, |el| {
                el.child(
                    div()
                        .id("workspace-sidebar-right")
                        .flex_none()
                        .w(px(200.))
                        .h_full()
                        .child(sidebar_entity.clone()),
                )
            })
    }
}
