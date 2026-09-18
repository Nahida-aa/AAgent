//! MultiWorkspace — 顶层窗口容器（对齐 zed `MultiWorkspace`）。
//!
//! Zed 的 MultiWorkspace 职责（multi_workspace.rs:L304-L322）：
//! - 管理多个 Workspace（窗口分屏）
//! - 持有 Sidebar dyn handle + sidebar_open/sidebar_side **状态**（自己存）
//! - Sidebar 独立于 Dock — 渲染顺序: [sidebar?] Workspace(main_area) [sidebar?]
//!
//! Zed 关键设计：sidebar_open + sidebar_side 是 MultiWorkspace 的字段，
//! 不是 Sidebar entity 的状态。Sidebar entity 只是内容容器，open/side 由
//! MultiWorkspace 控制（这样 SidebarHandle dyn 接口不需要状态字段）。

pub mod sidebar;
pub mod sidebar_handle;
pub mod sidebar_render_state;

pub use sidebar::Sidebar;
pub use sidebar_handle::SidebarHandle;
pub use sidebar_render_state::{SidebarRenderState, SidebarStatus};

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use crate::Workspace;
use settings_content::SidebarSide;

/// 顶层 MultiWorkspace entity。
///
/// Zed 对齐：
/// - multi_workspace.rs L316: `sidebar: Option<Box<dyn SidebarHandle>>` — dyn object
/// - multi_workspace.rs L317: `sidebar_open: bool` — **MultiWorkspace 自己存 open 状态**
/// - multi_workspace.rs L328: `sidebar_side()` — settings 层读 side
pub struct MultiWorkspace {
    workspace: Entity<Workspace>,
    sidebar: Option<Box<dyn SidebarHandle>>,
    /// **MultiWorkspace 自己持有 sidebar open 状态**（对齐 zed L317）。
    /// Sidebar entity 只是内容容器，不知道自己开没开。
    sidebar_open: bool,
    sidebar_side: SidebarSide,
}

impl MultiWorkspace {
    /// 创建 MultiWorkspace — 不含 Sidebar entity（对齐 zed L341-L380）。
    /// Sidebar entity 由 app 层创建后通过 set_sidebar() 注入。
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        // 订阅 workspace — StatusBar toggle_sidebar → MultiWorkspace 重渲染
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();

        Self {
            workspace,
            sidebar: None,
            sidebar_open: false,
            sidebar_side: SidebarSide::Left,
        }
    }

    /// 注入 Sidebar dyn handle — app 层创建 Sidebar entity 后调此方法。
    /// 对齐 zed 的模式：workspace crate 不依赖 sidebar crate（循环），
    /// 所以 MultiWorkspace 不创建 Sidebar entity，只接收 dyn handle。
    pub fn set_sidebar(&mut self, sidebar: Box<dyn SidebarHandle>, cx: &mut Context<Self>) {
        self.sidebar = Some(sidebar);
        cx.notify();
    }

    pub fn workspace(&self) -> &Entity<Workspace> {
        &self.workspace
    }

    pub fn sidebar(&self) -> Option<&dyn SidebarHandle> {
        self.sidebar.as_deref()
    }

    pub fn sidebar_open(&self) -> bool {
        self.sidebar_open
    }

    pub fn sidebar_side(&self, _cx: &App) -> SidebarSide {
        self.sidebar_side
    }

    pub fn sidebar_status(&self, _cx: &App) -> SidebarStatus {
        SidebarStatus {
            open: self.sidebar_open,
            side: self.sidebar_side,
        }
    }

    /// 只读渲染状态 — 对齐 zed L334-L338。
    pub fn sidebar_render_state(&self, _cx: &App) -> SidebarRenderState {
        SidebarRenderState {
            open: self.sidebar_open,
            side: self.sidebar_side,
        }
    }

    /// 对外 API — 切换 sidebar 开关（对齐 zed `ToggleWorkspaceSidebar`）。
    pub fn toggle_sidebar(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        if self.sidebar_side == side {
            self.sidebar_open = !self.sidebar_open;
        } else {
            self.sidebar_side = side;
            self.sidebar_open = true;
        }
        cx.notify();
    }

    /// 对外 API — 关闭 sidebar（对齐 zed `CloseWorkspaceSidebar`）。
    pub fn close_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = false;
        cx.notify();
    }

    pub fn set_sidebar_side(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        self.sidebar_side = side;
        self.sidebar_open = true;
        cx.notify();
    }
}

impl Render for MultiWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let has_left_sidebar = self.sidebar_open && self.sidebar_side == SidebarSide::Left;
        let has_right_sidebar = self.sidebar_open && self.sidebar_side == SidebarSide::Right;

        let sidebar_child = self.sidebar.as_ref().map(|s| s.to_any());

        div()
            .id("multi-workspace")
            .flex()
            .flex_row()
            .size_full()
            .bg(colors.panel_background)
            .when(has_left_sidebar, |el| {
                if let Some(sidebar) = sidebar_child.clone() {
                    el.child(
                        div()
                            .id("workspace-sidebar-left")
                            .flex_none()
                            .w(px(200.))
                            .h_full()
                            .child(sidebar),
                    )
                } else {
                    el
                }
            })
            .child(self.workspace.clone())
            .when(has_right_sidebar, |el| {
                if let Some(sidebar) = sidebar_child {
                    el.child(
                        div()
                            .id("workspace-sidebar-right")
                            .flex_none()
                            .w(px(200.))
                            .h_full()
                            .child(sidebar),
                    )
                } else {
                    el
                }
            })
    }
}
