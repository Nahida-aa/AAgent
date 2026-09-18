//! MultiWorkspace — 顶层窗口容器（对齐 zed `MultiWorkspace`）。
//!
//! Zed 的 MultiWorkspace 职责（multi_workspace.rs:L304-L322）：
//! - 管理多个 Workspace（窗口分屏）
//! - 持有 Sidebar dyn handle + sidebar_open/sidebar_side **状态**（自己存）
//! - Sidebar 独立于 Dock — 渲染顺序: [sidebar?] Workspace(main_area) [sidebar?]
//! - Sidebar 有 resize handle（对齐 zed L2110-L2160）

pub mod sidebar;
pub mod sidebar_handle;
pub mod sidebar_render_state;

pub use sidebar::Sidebar;
pub use sidebar_handle::SidebarHandle;
pub use sidebar_render_state::SidebarRenderState;

use gpui::{
    App, Context, DragMoveEvent, Entity, IntoElement, MouseButton, ParentElement, Render, Styled,
    Window, div, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use crate::Workspace;
use settings_content::SidebarSide;

/// Sidebar resize handle 的尺寸（宽 6px）— 对齐 zed L27。
const SIDEBAR_RESIZE_HANDLE_SIZE: gpui::Pixels = px(6.0);

/// Sidebar 拖拽类型 — 传给 `on_drag` / `on_drag_move`。
/// 对齐 zed `DraggedSidebar`（L180）。
#[derive(Clone)]
pub(crate) struct DraggedSidebar;

impl Render for DraggedSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let sidebar_side = self.sidebar_side;
        let sidebar_on_right = sidebar_side == SidebarSide::Right;

        // Sidebar container — 有 resize handle（对齐 zed L2110-L2160）。
        let sidebar = if self.sidebar_open {
            self.sidebar.as_ref().map(|sidebar_handle| {
                let weak = cx.weak_entity();
                let sidebar_width = sidebar_handle.width(cx);

                let resize_handle = div()
                    .id("sidebar-resize-handle")
                    .absolute()
                    .when(!sidebar_on_right, |el| {
                        el.right(-SIDEBAR_RESIZE_HANDLE_SIZE / 2.)
                    })
                    .when(sidebar_on_right, |el| {
                        el.left(-SIDEBAR_RESIZE_HANDLE_SIZE / 2.)
                    })
                    .top(px(0.))
                    .h_full()
                    .w(SIDEBAR_RESIZE_HANDLE_SIZE)
                    .cursor_col_resize()
                    .on_drag(DraggedSidebar, |dragged, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| dragged.clone())
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    // 双击 reset 宽度（对齐 zed L2140-L2150）
                    .on_mouse_up(MouseButton::Left, move |event, _, cx| {
                        if event.click_count == 2 {
                            weak.update(cx, |this, cx| {
                                if let Some(sidebar) = &this.sidebar {
                                    sidebar.set_width(None, cx);
                                }
                            })
                            .ok();
                            cx.stop_propagation();
                        }
                    });

                div()
                    .id("sidebar-container")
                    .relative()
                    .h_full()
                    .w(sidebar_width)
                    .flex_shrink_0()
                    .child(sidebar_handle.to_any())
                    .child(resize_handle)
                    .into_any_element()
            })
        } else {
            None
        };

        let (left_sidebar, right_sidebar) = if sidebar_on_right {
            (None, sidebar)
        } else {
            (sidebar, None)
        };

        div()
            .id("multi-workspace")
            .flex()
            .flex_row()
            .size_full()
            .bg(colors.panel_background)
            // Sidebar 宽度拖拽 — 对齐 zed L2252-L2267
            .when(self.sidebar_open, |this| {
                this.on_drag_move(cx.listener(
                    move |this: &mut Self, e: &DragMoveEvent<DraggedSidebar>, window, cx| {
                        if let Some(sidebar) = &this.sidebar {
                            let new_width = if sidebar_on_right {
                                window.bounds().size.width - e.event.position.x
                            } else {
                                e.event.position.x
                            };
                            sidebar.set_width(Some(new_width), cx);
                        }
                    },
                ))
            })
            .children(left_sidebar)
            .child(self.workspace.clone())
            .children(right_sidebar)
    }
}
