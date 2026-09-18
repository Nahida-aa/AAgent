//! Workspace 层：对齐 zed `workspace` crate。
//!
//! AAgent 主线（GPUI 桌面）的整体布局：
//!
//! ```
//! ┌───────────────────────────────────────┐
//! │ TitleBar                              │
//! ├───────┬───────────────────┬───────────┤
//! │       │                   │           │
//! │ Left  │   Center Pane    │   Right   │
//! │ Dock  │   (AgentPanel)   │   Dock    │
//! │       │                   │           │
//! ├───────┴───────────────────┴───────────┤
//! │ Bottom Dock                           │
//! ├───────────────────────────────────────┤
//! │ StatusBar (PanelButtons + 普通项)     │
//! └───────────────────────────────────────┘
//! ```
//!
//! 三层抽象（对齐 zed）：
//! - **Dock** — 面板容器（Left/Bottom/Right 三实例）
//! - **PanelButtons** — 状态栏上的 Dock 按钮，关联一个 Dock entity
//! - **Workspace** — 顶层 entity，持有 3 Dock + center pane + status bar

pub mod dock;
pub mod dock_position;
pub mod panel;
pub mod panel_buttons;
pub mod status_bar;

use std::collections::HashMap;

use gpui::{Context, Entity, ParentElement, Render, Styled, Window, div, hsla, prelude::*, px};
use ui_gpui::theme::ActiveTheme;

use dock::Dock;
use dock_position::DockPosition;
use panel::{PanelEntry, PanelKind};
use panel_buttons::PanelButtons;
use status_bar::StatusBar;

/// 顶层 Workspace entity。对齐 zed `Workspace` 但做了大幅简化。
/// Zed 的 Workspace ~3000 行（含 Pane、ItemHandle、ModalLayer、TeleportLayer 等），
/// AAgent 现阶段只持有 Dock + StatusBar + 中心区域。
pub struct Workspace {
    /// 三个 Dock 实例（左/底/右），每个装多个 Panel。
    left_dock: Entity<Dock>,
    bottom_dock: Entity<Dock>,
    right_dock: Entity<Dock>,
    /// 状态栏（含 PanelButtons + 普通状态项）。
    status_bar: Entity<StatusBar>,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 1. 创建 3 个 Dock（对齐 zed workspace.rs:L1970-L1972）
        let mut left_dock = Dock::new(DockPosition::Left);
        let mut bottom_dock = Dock::new(DockPosition::Bottom);
        let mut right_dock = Dock::new(DockPosition::Right);

        // 2. 注册面板到对应 Dock（PanelKind 自带 default_position）
        for kind in [
            PanelKind::Project,
            PanelKind::Git,
            PanelKind::Collab,
            PanelKind::Outline,
            PanelKind::Terminal,
            PanelKind::Debug,
            PanelKind::Agent,
        ] {
            let entry = PanelEntry::new(kind);
            match entry.default_position(Some(cx)) {
                DockPosition::Left => left_dock.add_panel(entry),
                DockPosition::Bottom => bottom_dock.add_panel(entry),
                DockPosition::Right => right_dock.add_panel(entry),
            }
        }

        // 3. 默认打开有面板的 Dock（Zed: Project Panel 在右默认打开；Agent 在左默认打开；Bottom 默认关）
        if !right_dock.panel_entries().is_empty() {
            right_dock.set_open(true);
            right_dock.activate_panel(0); // Project 是第一个
        }
        if !left_dock.panel_entries().is_empty() {
            left_dock.set_open(true);
            left_dock.activate_panel(0); // Agent 是第一个
        }
        // Bottom 默认关闭（Terminal 不自动弹出）

        let left_dock = cx.new(|cx| left_dock);
        let bottom_dock = cx.new(|cx| bottom_dock);
        let right_dock = cx.new(|cx| right_dock);

        // 订阅 3 个 dock — Dock toggle / 面板增删时 Workspace 需要重新渲染
        cx.observe(&left_dock, |_, _, cx| cx.notify()).detach();
        cx.observe(&bottom_dock, |_, _, cx| cx.notify()).detach();
        cx.observe(&right_dock, |_, _, cx| cx.notify()).detach();

        // 3. 创建 all_docks HashMap（PanelButtons 右键菜单跨 Dock 搬面板需要）
        let all_docks: HashMap<DockPosition, Entity<Dock>> = [
            (DockPosition::Left, left_dock.clone()),
            (DockPosition::Bottom, bottom_dock.clone()),
            (DockPosition::Right, right_dock.clone()),
        ]
        .into();

        // 4. 创建 3 个 PanelButtons（Dock 关联的状态栏按钮）
        let left_dock_buttons =
            cx.new(|cx| PanelButtons::new(left_dock.clone(), all_docks.clone(), cx));
        let bottom_dock_buttons =
            cx.new(|cx| PanelButtons::new(bottom_dock.clone(), all_docks.clone(), cx));
        let right_dock_buttons =
            cx.new(|cx| PanelButtons::new(right_dock.clone(), all_docks.clone(), cx));

        // 4. 创建 StatusBar + 组装
        let status_bar = cx.new(|cx| {
            let mut bar = StatusBar::new(cx);

            // 先加 PanelButtons（对齐 zed workspace.rs:L1983-L1985）
            bar.add_left_item(left_dock_buttons);
            bar.add_right_item(right_dock_buttons);
            bar.add_right_item(bottom_dock_buttons);

            // 再对齐 zed 的普通状态项
            use status_bar::items::{
                ActiveFileName, ActivityIndicator, CursorPosition, Diagnostics, EditPrediction,
                Encoding, GitBlame, ImageInfo, Language, LanguageServers, LineEnding,
                MergeConflict, PendingKeystrokes, Search, Toolchain, VimMode,
            };
            // 左组
            bar.add_left_item(cx.new(|cx| Search::new(cx)));
            bar.add_left_item(cx.new(|cx| LanguageServers::new(cx)));
            bar.add_left_item(cx.new(|cx| Diagnostics::new(cx)));
            bar.add_left_item(cx.new(|cx| ActiveFileName::empty(cx)));
            bar.add_left_item(cx.new(|cx| GitBlame::new(cx)));
            bar.add_left_item(cx.new(|cx| MergeConflict::new(cx)));
            bar.add_left_item(cx.new(|cx| ActivityIndicator::new(cx)));
            // 右组（反序渲染）
            bar.add_right_item(cx.new(|cx| EditPrediction::new(cx)));
            bar.add_right_item(cx.new(|cx| Encoding::new(cx)));
            bar.add_right_item(cx.new(|cx| Language::new(cx)));
            bar.add_right_item(cx.new(|cx| Toolchain::new(cx)));
            bar.add_right_item(cx.new(|cx| LineEnding::new(cx)));
            bar.add_right_item(cx.new(|cx| CursorPosition::new(cx)));
            bar.add_right_item(cx.new(|cx| ImageInfo::new(cx)));
            bar.add_right_item(cx.new(|cx| VimMode::new(cx)));
            bar.add_right_item(cx.new(|cx| PendingKeystrokes::new(cx)));

            bar
        });

        // 订阅 status_bar — Sidebar toggle 改变 sidebar.open 时需要重渲染
        cx.observe(&status_bar, |_, _, cx| cx.notify()).detach();

        Self {
            left_dock,
            bottom_dock,
            right_dock,
            status_bar,
        }
    }

    pub fn status_bar(&self) -> &Entity<StatusBar> {
        &self.status_bar
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        // 读取三个 dock 的开/关状态（决定是否占空间）
        let left_open = self.left_dock.read(cx).is_open();
        let right_open = self.right_dock.read(cx).is_open();
        let bottom_open = self.bottom_dock.read(cx).is_open();

        // 读取 sidebar 状态（StatusBar 里的 SidebarStatus）
        let sidebar = self.status_bar.read(cx).sidebar();
        let sidebar_left = sidebar.open && sidebar.side == DockPosition::Left;
        let sidebar_right = sidebar.open && sidebar.side == DockPosition::Right;

        // Sidebar 占位内容（后续换成文件树）
        let sidebar_content = |pos: DockPosition| {
            div()
                .w(px(220.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.panel_background)
                .border_color(colors.border)
                .text_size(px(13.0))
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .map(|el| match pos {
                    DockPosition::Left => el.border_r_1(),
                    DockPosition::Right => el.border_l_1(),
                    _ => el,
                })
                .child(format!("Sidebar ({:?})", pos))
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.panel_background)
            .overflow_hidden()
            // 主区域（flex_1 占满剩余空间）
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(colors.background)
                    .overflow_hidden()
                    // 上半部分：flex_row（sidebar-left | left dock | center | right dock | sidebar-right）
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .size_full()
                            .overflow_hidden()
                            // Sidebar（左侧，在 Left Dock 左边）
                            .when(sidebar_left, |el| {
                                el.child(sidebar_content(DockPosition::Left))
                            })
                            // Left Dock（打开时才占空间）
                            .when(left_open, |el| {
                                el.child(div().w(px(280.0)).h_full().child(self.left_dock.clone()))
                            })
                            // Center placeholder
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(14.0))
                                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                    .child("AAgent Center (placeholder)"),
                            )
                            // Right Dock（打开时才占空间）
                            .when(right_open, |el| {
                                el.child(div().w(px(280.0)).h_full().child(self.right_dock.clone()))
                            })
                            // Sidebar（右侧，在 Right Dock 右边）
                            .when(sidebar_right, |el| {
                                el.child(sidebar_content(DockPosition::Right))
                            }),
                    )
                    // Bottom Dock（打开时才占空间，叠在底部）
                    .when(bottom_open, |el| {
                        el.child(div().w_full().h(px(240.0)).child(self.bottom_dock.clone()))
                    }),
            )
            // StatusBar
            .child(self.status_bar.clone())
    }
}
