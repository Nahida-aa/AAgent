//! Sidebar entity — Agent Threads 列表 + 底部栏。
//!
//! 对齐 zed `crates/sidebar/src/sidebar.rs`。
//! 独立 crate 后，workspace crate 通过 `Box<dyn SidebarHandle>` 持有
//! （dyn object 解耦循环依赖），sidebar crate 单向依赖 workspace crate
//! （Sidebar trait 定义在 workspace::multi_workspace::sidebar）。
//!
//! 渲染结构（对齐 zed L7914-L8043）：
//! ```
//! v_flex [
//!   header (Project name + filter)     ← 后续实现
//!   list (Thread / Terminal entries)   ← 后续实现
//!   render_sidebar_bottom_bar          ← 现在实现
//!     [sidebar-close] [archive-toggle] (flex_1) [add-project]
//! ]
//! ```

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use settings_content::SidebarSide;
use ui_gpui::IconName;
use ui_gpui::component::tooltip::Tooltip;
use ui_gpui::theme::ActiveTheme;
use ui_gpui::{ButtonRadius, IconButton};

use workspace::MultiWorkspace;
use workspace::multi_workspace::sidebar::Sidebar as SidebarTrait;

/// Sidebar entity — **内容容器**，不知道自己开没开（那是 MultiWorkspace 的状态）。
///
/// Zed 关键设计：open/side 状态由 MultiWorkspace 持有，Sidebar entity 只负责
/// 渲染内容。Sidebar entity 内部仍持有 `side` 字段（用于 border 渲染），
/// 但 MultiWorkspace 的 `sidebar_open` 字段决定 Sidebar 是否被渲染出来。
pub struct Sidebar {
    /// sidebar 在左还是右 — entity 自己存（用于 border 渲染 + Sidebar trait side() getter）。
    /// 由 App 层创建后通过 set_side() 设置。
    side: SidebarSide,
    /// 是否显示 archive/history 视图（对齐 zed SidebarView::Archive）。
    show_archive: bool,
    /// sidebar 宽度（默认 200px，对齐 zed sidebar width）。
    width: gpui::Pixels,
    /// 弱引用 MultiWorkspace（关闭按钮需要反向调用 MultiWorkspace.close_sidebar()）。
    /// sidebar crate 单向依赖 workspace crate，所以 WeakEntity<MultiWorkspace> 合法。
    multi_workspace: Option<WeakEntity<MultiWorkspace>>,
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            side: SidebarSide::Left,
            show_archive: false,
            width: px(200.),
            multi_workspace: None,
        }
    }

    /// App 层创建后设置 MultiWorkspace 弱引用 — 关闭按钮回调需要。
    pub fn set_multi_workspace(&mut self, mw: Entity<MultiWorkspace>) {
        self.multi_workspace = Some(mw.downgrade());
    }

    /// App 层设置 sidebar 在哪一侧 — 影响 border 渲染。
    pub fn set_side(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        self.side = side;
        cx.notify();
    }

    pub fn side(&self) -> SidebarSide {
        self.side
    }

    fn toggle_archive(&mut self, cx: &mut Context<Self>) {
        self.show_archive = !self.show_archive;
        cx.notify();
    }

    /// 底部栏 — 对齐 zed L7473-L7502。
    ///
    /// ```
    /// ┌──────────────────────────────────────────────┐
    /// │ [sidebar-close] [archive-toggle]  (flex_1)  [add-project] │
    /// └──────────────────────────────────────────────┘
    /// ```
    fn render_bottom_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let is_archive = self.show_archive;
        let on_right = self.side == SidebarSide::Right;
        let sidebar_open_icon = if on_right {
            IconName::ThreadsSidebarRightOpen
        } else {
            IconName::ThreadsSidebarLeftOpen
        };

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
            // 1. Sidebar close toggle — 调 MultiWorkspace.close_sidebar()
            .child({
                let mw = self.multi_workspace.clone();
                IconButton::new("sidebar-close-toggle", sidebar_open_icon)
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
            // 2. History toggle — Clock 图标（对齐 zed IconName::Clock）
            .child({
                let archive = is_archive;
                let base = IconButton::new("sidebar-toggle-archive", IconName::Clock)
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
            // 4. Open Project — FolderAdd 图标（对齐 zed render_recent_projects_button）
            .child({
                IconButton::new("sidebar-add-project", IconName::FolderAdd)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ButtonRadius::Medium)
                    .aria_label("Open Project")
                    .tooltip(Tooltip::text("Open Project"))
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
            .when(self.side == SidebarSide::Left, |el| el.border_r_1())
            .when(self.side == SidebarSide::Right, |el| el.border_l_1())
            .border_color(colors.border)
            // Thread list (placeholder)
            .child(placeholder_list)
            // Bottom bar
            .child(self.render_bottom_bar(cx))
    }
}

// Sidebar entity 实现 workspace::Sidebar trait — 桥接 SidebarHandle dyn object。
impl SidebarTrait for Sidebar {
    fn width(&self, _cx: &App) -> gpui::Pixels {
        self.width
    }

    fn set_width(&mut self, width: Option<gpui::Pixels>, _cx: &mut Context<Self>) {
        if let Some(w) = width {
            self.width = w.max(px(120.)); // 最小 120px
        } else {
            self.width = px(200.); // reset 默认
        }
    }

    fn has_notifications(&self, _cx: &App) -> bool {
        false // AAgent 暂时没有未读通知机制
    }

    fn side(&self, _cx: &App) -> SidebarSide {
        self.side
    }
}
