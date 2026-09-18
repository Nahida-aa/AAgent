//! Workspace 层：对齐 zed `workspace` crate。
//!
//! AAgent 主线（GPUI 桌面）的整体布局 — 完全照搬 Zed 的 4 种 BottomDockLayout：
//!
//! ```
//! Contained (默认):           Full:
//! ┌──────┬───────┬──────┐    ┌───────────────────────────┐
//! │ Left │Center │Right │    │ Left  │   Center   │ Right │
//! │ Dock │Pane   │ Dock  │    │ Dock  │   Pane     │ Dock  │
//! │      ├───────┤       │    ├──────┴───┬───────┴──────┤
//! │      │Bottom │       │    │   Bottom Dock (全宽)    │
//! │      │ Dock  │       │    └───────────────────────────┘
//! └──────┴───────┴──────┘
//! ```
//!
//! 三层抽象（对齐 zed）：
//! - **Dock** — 面板容器（Left/Bottom/Right 三实例），内置 resize handle
//! - **PanelButtons** — 状态栏上的 Dock 按钮，关联一个 Dock entity
//! - **Workspace** — 顶层 entity，持有 3 Dock + center pane + status bar，
//!   顶层 div 监听 `on_drag_move<DraggedDock>` 接收所有 Dock resize 拖拽事件

pub mod dock;
pub mod multi_workspace;
pub mod sidebar;
pub mod status_bar;

pub use multi_workspace::MultiWorkspace;
pub use settings_content::DockPosition;
pub use sidebar::{Sidebar, SidebarStatus};

use std::collections::HashMap;

use gpui::{
    App, Axis, Bounds, Context, DragMoveEvent, Entity, IntoElement, ParentElement, Render, Styled,
    Window, canvas, div, hsla, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use dock::panel::{
    AgentPanel, CollabPanel, DebugPanel, GitPanel, OutlinePanel, PanelHandle, ProjectPanel,
    TerminalPanel,
};
use dock::panel_buttons::PanelButtons;
use dock::{Dock, DraggedDock};
use status_bar::StatusBar;

/// Bottom dock 布局（对齐 zed `settings_content/src/workspace.rs`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomDockLayout {
    /// 夹在左右 dock 之间（默认）
    Contained,
    /// 窗口全宽
    Full,
    /// 左侧与 Left Dock 对齐，右侧夹在中间区域
    LeftAligned,
    /// 右侧与 Right Dock 对齐，左侧夹在中间区域
    RightAligned,
}

impl Default for BottomDockLayout {
    fn default() -> Self {
        Self::Contained
    }
}

impl BottomDockLayout {
    fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "full" => Self::Full,
            "left_aligned" => Self::LeftAligned,
            "right_aligned" => Self::RightAligned,
            _ => Self::Contained,
        }
    }
}

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
    /// Workspace 边界 — 用于 resize 计算右 dock / 底 dock 的尺寸。
    /// 通过 canvas element 更新（对齐 zed workspace.rs:L9669-L9706）。
    bounds: Bounds<gpui::Pixels>,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        use std::sync::Arc;

        // 1. 创建 3 个 Dock（对齐 zed workspace.rs:L1970-L1972）
        let mut left_dock = Dock::new(DockPosition::Left);
        let mut bottom_dock = Dock::new(DockPosition::Bottom);
        let mut right_dock = Dock::new(DockPosition::Right);

        // 2. 创建 7 个 Panel Entity（每个是独立的 GPUI entity），
        //    按各自默认位置注册到对应 Dock。
        //
        // Zed default.json 布局:
        // - Project/Git/Collab/Outline → Right
        // - Agent → Left
        // - Terminal/Debug → Bottom
        let mut add_panel_to_dock = |dock: &mut Dock, panel: Arc<dyn PanelHandle>| {
            dock.add_panel(panel);
        };

        // Left Dock — Agent
        let agent = Arc::new(cx.new(|_| AgentPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut left_dock, agent);

        // Right Dock — Project, Git, Collab, Outline
        let project = Arc::new(cx.new(|_| ProjectPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut right_dock, project);

        let git = Arc::new(cx.new(|_| GitPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut right_dock, git);

        let collab = Arc::new(cx.new(|_| CollabPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut right_dock, collab);

        let outline = Arc::new(cx.new(|_| OutlinePanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut right_dock, outline);

        // Bottom Dock — Terminal, Debug
        let terminal = Arc::new(cx.new(|_| TerminalPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut bottom_dock, terminal);

        let debug = Arc::new(cx.new(|_| DebugPanel)) as Arc<dyn PanelHandle>;
        add_panel_to_dock(&mut bottom_dock, debug);

        // 3. Dock 默认全关（对齐 Zed Dock::new 硬编码 is_open: false）。
        // Zed 没有 default.json 里的 open 字段 — 首次启动全关，
        // 但 Panel trait 有 starts_open() 方法（Project 面板默认为 true）。
        // 现阶段简化为全关 — 后续加 Panel trait 时支持 starts_open。

        let left_dock = cx.new(|_| left_dock);
        let bottom_dock = cx.new(|_| bottom_dock);
        let right_dock = cx.new(|_| right_dock);

        // 订阅 3 个 dock — Dock toggle / 面板增删时 Workspace 需要重新渲染
        cx.observe(&left_dock, |_, _, cx| cx.notify()).detach();
        cx.observe(&bottom_dock, |_, _, cx| cx.notify()).detach();
        cx.observe(&right_dock, |_, _, cx| cx.notify()).detach();

        // 4. 创建 all_docks HashMap（PanelButtons 右键菜单跨 Dock 搬面板需要）
        let all_docks: HashMap<DockPosition, Entity<Dock>> = [
            (DockPosition::Left, left_dock.clone()),
            (DockPosition::Bottom, bottom_dock.clone()),
            (DockPosition::Right, right_dock.clone()),
        ]
        .into();

        // 5. 创建 3 个 PanelButtons（Dock 关联的状态栏按钮）
        let left_dock_buttons =
            cx.new(|cx| PanelButtons::new(left_dock.clone(), all_docks.clone(), cx));
        let bottom_dock_buttons =
            cx.new(|cx| PanelButtons::new(bottom_dock.clone(), all_docks.clone(), cx));
        let right_dock_buttons =
            cx.new(|cx| PanelButtons::new(right_dock.clone(), all_docks.clone(), cx));

        // 6. 创建 StatusBar + 组装
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
            bounds: Bounds::default(),
        }
    }

    pub fn status_bar(&self) -> &Entity<StatusBar> {
        &self.status_bar
    }

    /// MultiWorkspace 创建 Sidebar 后传给 Workspace，Workspace 传给 StatusBar。
    /// StatusBar toggle sidebar 时直接调 Sidebar entity。
    pub fn set_sidebar_entity(
        &mut self,
        sidebar: &Entity<crate::sidebar::Sidebar>,
        cx: &mut Context<Self>,
    ) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_sidebar_entity(sidebar.clone());
            bar.set_sidebar(sidebar.read(cx).status());
            cx.notify();
        });
    }

    /// 对齐 zed `resize_left_dock` — 调整左 dock 宽度。
    fn resize_left_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        self.left_dock.update(cx, |dock, cx| {
            dock.set_size(new_size);
            cx.notify();
        });
    }

    /// 对齐 zed `resize_right_dock` — 调整右 dock 宽度。
    fn resize_right_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        self.right_dock.update(cx, |dock, cx| {
            dock.set_size(new_size);
            cx.notify();
        });
    }

    /// 对齐 zed `resize_bottom_dock` — 调整底部 dock 高度。
    fn resize_bottom_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        self.bottom_dock.update(cx, |dock, cx| {
            dock.set_size(new_size);
            cx.notify();
        });
    }

    /// 对齐 zed `render_dock` — 返回 Dock 容器。
    ///
    /// 关键点：即使 Dock 关闭也返回容器（flex_none 不占空间），
    /// 保证 focus handle 始终挂载。
    ///
    /// 当 Dock 的 active panel 支持 flexible sizing（Agent 面板）且没有用户覆盖的固定尺寸时，
    /// 用 flex_grow(1.0) 让它和 Center 等宽（对齐 zed `default_dock_flex` 返回 1.0）。
    fn render_dock(dock: &Entity<Dock>, position: DockPosition, cx: &App) -> impl IntoElement {
        let dock_ref = dock.read(cx);
        let is_open = dock_ref.is_open();
        let size = dock_ref.current_size(cx);

        // 判断是否 flexible sizing：
        let is_flexible = match position {
            DockPosition::Left | DockPosition::Right => {
                !dock_ref.has_size_override()
                    && dock_ref
                        .active_panel_index()
                        .and_then(|i| dock_ref.panels().get(i))
                        .map(|p| p.supports_flexible_size(cx))
                        .unwrap_or(false)
            }
            DockPosition::Bottom => false,
        };

        let id = match position {
            DockPosition::Left => "left-dock",
            DockPosition::Right => "right-dock",
            DockPosition::Bottom => "bottom-dock",
        };

        let mut container = div().id(id).overflow_hidden().child(dock.clone());

        if is_open {
            if is_flexible && matches!(position, DockPosition::Left | DockPosition::Right) {
                // Flexible sizing — 和 Center 1:1 等宽。
                // 关键：flex_basis(0) — 没有它默认 flex-basis:auto 会按内容大小分配，
                // 不是等分（对齐 zed workspace.rs:L8696 + gpui flex_1() 的实现）。
                container = container
                    .flex_grow(1.0)
                    .flex_shrink(1.0)
                    .flex_basis(gpui::relative(0.));
            } else {
                match position {
                    DockPosition::Left | DockPosition::Right => {
                        container = container.w(px(size)).flex_shrink(1.0);
                    }
                    DockPosition::Bottom => {
                        container = container.h(px(size));
                    }
                }
            }
        } else {
            container = container.flex_none();
        }

        container
    }

    /// 从 SettingsStore 读取 bottom_dock_layout（默认 Contained）。
    fn read_bottom_dock_layout(cx: &App) -> BottomDockLayout {
        if let Some(store) = cx.try_global::<settings::SettingsStore>() {
            if let Ok(s) = store.try_get_path::<String>(&["bottom_dock_layout"]) {
                return BottomDockLayout::from_str(&s);
            }
        }
        BottomDockLayout::default()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        let left_dock = Self::render_dock(&self.left_dock, DockPosition::Left, cx);
        let right_dock = Self::render_dock(&self.right_dock, DockPosition::Right, cx);
        let bottom_dock = Self::render_dock(&self.bottom_dock, DockPosition::Bottom, cx);

        // Center placeholder — 后续换成真实 Pane/PaneGroup
        let center = div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .text_size(px(14.0))
            .text_color(hsla(0.0, 0.0, 0.5, 1.0))
            .child("AAgent Center (placeholder)");

        // 读取 bottom dock 布局
        let bottom_layout = Self::read_bottom_dock_layout(cx);

        // 主区域 — 4 种布局树（对齐 zed workspace.rs:L9748-L9982）
        let main_area = match bottom_layout {
            // ┌──────┬──────────┬──────┐
            // │ Left │  Center  │Right │
            // │ Dock │  ┌──────┐│ Dock │
            // │      │  │Bottom││      │
            // │      │  └──────┘│      │
            // └──────┴──────────┴──────┘
            BottomDockLayout::Contained => div()
                .flex()
                .flex_row()
                .h_full()
                .child(left_dock)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .h_full()
                        .overflow_hidden()
                        .child(center)
                        .child(bottom_dock),
                )
                .child(right_dock)
                .into_any_element(),

            // ┌──────────────────────────┐
            // │ Left  │   Center   │Right │
            // │ Dock  │            │ Dock  │
            // ├───────┴────────────┴──────┤
            // │     Bottom Dock (全宽)     │
            // └──────────────────────────┘
            BottomDockLayout::Full => div()
                .flex()
                .flex_col()
                .h_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .overflow_hidden()
                        .child(left_dock)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .overflow_hidden()
                                .child(center),
                        )
                        .child(right_dock),
                )
                .child(bottom_dock)
                .into_any_element(),

            // ┌──────┬─────────────────────┐
            // │ Left │      Center         │
            // │ Dock │  ┌──────────────┐   │
            // │      │  │   Bottom     │   │
            // │      │  └──────────────┘   │
            // └──────┴─────────────────────┘
            BottomDockLayout::LeftAligned => div()
                .flex()
                .flex_row()
                .h_full()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .h_full()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_1()
                                .overflow_hidden()
                                .child(left_dock)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .overflow_hidden()
                                        .child(center),
                                ),
                        )
                        .child(bottom_dock),
                )
                .child(right_dock)
                .into_any_element(),

            // ┌─────────────────────┬──────┐
            // │      Center         │Right │
            // │  ┌──────────────┐   │ Dock │
            // │  │   Bottom     │   │      │
            // │  └──────────────┘   │      │
            // └─────────────────────┴──────┘
            BottomDockLayout::RightAligned => div()
                .flex()
                .flex_row()
                .h_full()
                .child(left_dock)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .h_full()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_1()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .overflow_hidden()
                                        .child(center),
                                )
                                .child(right_dock),
                        )
                        .child(bottom_dock),
                )
                .into_any_element(),
        };

        let this = cx.entity();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.panel_background)
            .overflow_hidden()
            // canvas — 每帧更新 bounds（对齐 zed workspace.rs:L9669-L9706）
            .child(
                canvas(
                    move |bounds, _, cx| {
                        this.update(cx, |workspace, cx| {
                            workspace.bounds = bounds;
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            // 顶层 on_drag_move listener — 接收所有 Dock resize 拖拽事件
            // 对齐 zed workspace.rs:L9707-L9740
            .on_drag_move::<DraggedDock>(cx.listener(
                move |workspace: &mut Self,
                      e: &DragMoveEvent<DraggedDock>,
                      _window: &mut Window,
                      cx| {
                    let bounds = workspace.bounds;
                    let pos = e.event.position;
                    match e.drag(cx).0 {
                        DockPosition::Left => {
                            workspace.resize_left_dock(pos.x.as_f32() - bounds.left().as_f32(), cx);
                        }
                        DockPosition::Right => {
                            workspace
                                .resize_right_dock(bounds.right().as_f32() - pos.x.as_f32(), cx);
                        }
                        DockPosition::Bottom => {
                            workspace
                                .resize_bottom_dock(bounds.bottom().as_f32() - pos.y.as_f32(), cx);
                        }
                    }
                },
            ))
            .child(main_area)
            // StatusBar
            .child(self.status_bar.clone())
    }
}
