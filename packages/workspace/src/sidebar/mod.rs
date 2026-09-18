//! Sidebar entity — Agent Threads 列表 + 底部栏。
//!
//! 对齐 zed `crates/sidebar/src/sidebar.rs`。
//! Zed Sidebar 在独立 crate，但 AAgent 现阶段放 workspace 子目录。
//!
//! 渲染结构（对齐 zed L7914-L8043）：
//! ```
//! v_flex [
//!   header (Project name + filter)     ← 后续实现
//!   list (Thread / Terminal entries)   ← 后续实现
//!   render_sidebar_bottom_bar          ← 现在实现
//!     [sidebar-toggle] [archive-toggle] (flex_1) [add-project]
//! ]
//! ```

use gpui::{
    App, Context, Entity, EntityId, IntoElement, ParentElement, Render, Styled, WeakEntity, Window,
    div, prelude::*, px,
};
use settings_content::SidebarSide;
use ui_gpui::IconName;
use ui_gpui::component::tooltip::Tooltip;
use ui_gpui::theme::ActiveTheme;
use ui_gpui::{ButtonRadius, IconButton};

use crate::multi_workspace::MultiWorkspace;

/// Sidebar 开关状态（对齐 zed `SidebarStatus{open, side: SidebarSide}`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SidebarStatus {
    pub open: bool,
    pub side: SidebarSide,
}

impl Default for SidebarStatus {
    fn default() -> Self {
        Self {
            open: false,
            side: SidebarSide::Left,
        }
    }
}

/// Sidebar entity — MultiWorkspace 持有。
pub struct Sidebar {
    status: SidebarStatus,
    /// 是否显示 archive/history 视图（对齐 zed SidebarView::Archive）。
    show_archive: bool,
    /// 弱引用 MultiWorkspace（close_sidebar 需要反向调用 MultiWorkspace 方法）。
    /// 用 WeakEntity 避免循环引用（MultiWorkspace → Sidebar → MultiWorkspace）。
    multi_workspace: Option<WeakEntity<MultiWorkspace>>,
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            status: SidebarStatus::default(),
            show_archive: false,
            multi_workspace: None,
        }
    }

    pub fn set_multi_workspace(&mut self, mw: Entity<MultiWorkspace>) {
        self.multi_workspace = Some(mw.downgrade());
    }

    pub fn status(&self) -> SidebarStatus {
        self.status
    }

    pub fn open(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        self.status.open = true;
        self.status.side = side;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.status.open = false;
        cx.notify();
    }

    pub fn toggle(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        if self.status.open && self.status.side == side {
            self.status.open = false;
        } else {
            self.status.open = true;
            self.status.side = side;
        }
        cx.notify();
    }

    pub fn set_side(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        self.status.side = side;
        cx.notify();
    }

    fn toggle_archive(&mut self, cx: &mut Context<Self>) {
        self.show_archive = !self.show_archive;
        cx.notify();
    }

    /// 底部栏 — 对齐 zed L7473-L7502。
    ///
    /// ```
    /// ┌──────────────────────────────────────────┐
    /// │ [sidebar-toggle] [archive-toggle]  (flex_1)  [add-project] │
    /// └──────────────────────────────────────────┘
    /// ```
    fn render_bottom_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let is_archive = self.show_archive;
        let on_right = self.status.side == SidebarSide::Right;
        let mw = self.multi_workspace.clone();

        let colors = cx.theme().colors();

        div()
            .id("sidebar-bottom-bar")
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_1()
            .py_1()
            .border_t_1()
            .border_color(colors.border)
            .when(on_right, |this| this.flex_row_reverse())
            // 1. Sidebar toggle — 关闭按钮
            .child({
                let mw = self.multi_workspace.clone();
                IconButton::new("sidebar-close-toggle", IconName::ThreadsSidebarLeftClosed)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ButtonRadius::Medium)
                    .aria_label("Close Sidebar")
                    .tooltip(Tooltip::text("Close Sidebar"))
                    .tooltip_anchor(gpui::Anchor::BottomLeft)
                    .tooltip_attach(gpui::Anchor::TopLeft)
                    .on_click(move |_ev, _window, cx| {
                        if let Some(mw) = mw.as_ref() {
                            if let Some(mw_entity) = mw.upgrade() {
                                mw_entity.update(cx, |mw, cx| {
                                    mw.close_sidebar(cx);
                                });
                            }
                        }
                    })
                    .into_any_element()
            })
            // 2. Archive toggle — 归档/历史
            .child({
                let archive = is_archive;
                let base = IconButton::new("sidebar-toggle-archive", IconName::Minimize)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ButtonRadius::Medium)
                    .aria_label(if archive {
                        "Hide Thread History"
                    } else {
                        "Show Thread History"
                    })
                    .tooltip(Tooltip::text(if archive {
                        "Hide Thread History"
                    } else {
                        "Show Thread History"
                    }))
                    .tooltip_anchor(gpui::Anchor::BottomLeft)
                    .tooltip_attach(gpui::Anchor::TopLeft)
                    .selected(archive)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_archive(cx);
                    }));
                base.into_any_element()
            })
            // 3. flex_1 spacer
            .child(div().flex_1())
            // 4. Add Project
            .child({
                IconButton::new("sidebar-add-project", IconName::Menu)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ButtonRadius::Medium)
                    .aria_label("Add Project")
                    .tooltip(Tooltip::text("Add Project"))
                    .tooltip_anchor(gpui::Anchor::BottomRight)
                    .tooltip_attach(gpui::Anchor::TopRight)
                    .into_any_element()
            })
            .into_any_element()
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        // 占位 thread list — 后续实现真实 Agent Threads
        let placeholder_list = div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .w_full()
            .h_full()
            .overflow_hidden()
            .text_size(px(13.0))
            .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
            .child("Agent Threads (placeholder)")
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                    .pt_1()
                    .child("— empty —"),
            );

        div()
            .id("workspace-sidebar")
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.surface_background)
            .when(self.status.side == SidebarSide::Left, |el| el.border_r_1())
            .when(self.status.side == SidebarSide::Right, |el| el.border_l_1())
            .border_color(colors.border)
            // Thread list (placeholder)
            .child(placeholder_list)
            // Bottom bar
            .child(self.render_bottom_bar(cx))
    }
}
