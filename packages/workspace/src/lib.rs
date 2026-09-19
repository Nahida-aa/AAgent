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
pub mod item;
pub mod multi_workspace;
pub mod pane;
pub mod pane_group;
pub mod status_bar;
pub mod terminal;

pub use item::{Item, ItemHandle, WeakItemHandle};
pub use multi_workspace::{MultiWorkspace, SidebarHandle, SidebarRenderState};
pub use pane::{DraggedSelection, DraggedTab, Event as PaneEvent, Pane};
pub use pane_group::{Member, PaneGroup};
pub use settings_content::DockPosition;
pub use terminal::{NewCenterTerminal, NewTerminal, OpenTerminal, TerminalProvider};

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    Action, App, Axis, Bounds, Context, DragMoveEvent, Entity, IntoElement, ParentElement, Render,
    Styled, Window, canvas, div, hsla, prelude::*, px,
};
use ui_gpui::StyledExt;
use ui_gpui::theme::ActiveTheme;

use dock::panel::{
    AgentPanel, CollabPanel, DebugPanel, GitPanel, OutlinePanel, Panel, PanelHandle, ProjectPanel,
};
use dock::panel_buttons::PanelButtons;
use dock::{Dock, DraggedDock, RESIZE_HANDLE_SIZE};
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
    /// 中心 PaneGroup — 递归 split 树，装 Pane（每个 Pane 装 items）。
    center: PaneGroup,
    /// 状态栏（含 PanelButtons + 普通状态项）。
    status_bar: Entity<StatusBar>,
    /// Workspace 边界 — 用于 resize 计算右 dock / 底 dock 的尺寸。
    /// 通过 canvas element 更新（对齐 zed workspace.rs:L9669-L9706）。
    bounds: Bounds<gpui::Pixels>,
    /// 对齐 zed `previous_dock_drag_coordinates` — 坐标去重, 避免相同位置重复触发 resize。
    previous_dock_drag_coordinates: Option<gpui::Point<gpui::Pixels>>,
    /// 外部注册的 action callback 收集器。
    /// Zed 用 `Vec<Box<dyn Fn(Div, ...) -> Div>>` 在 render 顶层 div 上应用。
    /// 我们简化为 `Vec<Box<dyn ActionCallback>>`，render 时 chain on_action。
    workspace_actions: Vec<Box<dyn ActionCallback>>,
    /// 可选的窗口装饰（TitleBar）— 由外部 crate（title-bar）创建后注入。
    /// 对齐 zed `workspace.rs:1598 titlebar_item: Option<AnyView>`。
    titlebar_item: Option<gpui::AnyView>,
}

/// 类型擦除的 action callback — 包装 `impl Fn(&mut Self, &A, ...)`。
trait ActionCallback: 'static {
    fn apply(&self, div: gpui::Div, cx: &mut Context<Workspace>) -> gpui::Div;
}

struct TypedActionCallback<
    A: Action,
    F: Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>) + 'static,
> {
    callback: Arc<F>,
    _marker: std::marker::PhantomData<A>,
}

impl<A: Action, F: Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>) + 'static>
    ActionCallback for TypedActionCallback<A, F>
{
    fn apply(&self, div: gpui::Div, cx: &mut Context<Workspace>) -> gpui::Div {
        let cb = self.callback.clone();
        div.on_action(
            cx.listener(move |workspace, event, window, cx| cb(workspace, event, window, cx)),
        )
    }
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 1. 创建 3 个 Dock（对齐 zed workspace.rs:L1970-L1972）
        // Zed: Dock::new() 都是空的 — 面板统一在 initialize_panels 里注入
        let left_dock = Dock::new(DockPosition::Left);
        let bottom_dock = Dock::new(DockPosition::Bottom);
        let right_dock = Dock::new(DockPosition::Right);

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

        // 7. 创建初始 center PaneGroup（一个空 Pane）
        let initial_pane = cx.new(Pane::new);
        let center = PaneGroup::new(initial_pane);

        Self {
            left_dock,
            bottom_dock,
            right_dock,
            center,
            status_bar,
            bounds: Bounds::default(),
            previous_dock_drag_coordinates: None,
            workspace_actions: Vec::new(),
            titlebar_item: None,
        }
    }

    pub fn status_bar(&self) -> &Entity<StatusBar> {
        &self.status_bar
    }

    /// 注入窗口装饰（TitleBar）— 由外部 app 层在 observe_new 里调。
    /// 对齐 zed `Workspace::set_titlebar_item`。
    pub fn set_titlebar_item(&mut self, item: gpui::AnyView, cx: &mut Context<Self>) {
        self.titlebar_item = Some(item);
        cx.notify();
    }

    pub fn titlebar_item(&self) -> Option<gpui::AnyView> {
        self.titlebar_item.clone()
    }

    /// 注册 action handler — 外部 crate（terminal-view 等）在 observe_new 里调用。
    /// 对齐 Zed `Workspace::register_action::<A>(callback)`。
    pub fn register_action<A: Action>(
        &mut self,
        callback: impl Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>) + 'static,
    ) {
        self.workspace_actions
            .push(Box::new(TypedActionCallback::<A, _> {
                callback: Arc::new(callback),
                _marker: std::marker::PhantomData,
            }));
    }

    // ---------- 泛型 Panel 操作（对齐 Zed Workspace::add_panel / toggle_panel_focus）----------
    //
    // 这些方法让外部 crate（terminal-view 等）能注册自己的 Panel 类型，
    // Workspace 不需要知道具体类型 — 通过泛型 T: Panel 抽象。

    /// 按 Panel 默认位置注入到对应 Dock。
    /// 对齐 zed `Workspace::add_panel::<T>` — Zed 在 Dock::add_panel 内部
    /// 调 `panel.read(cx).starts_open()` 决定是否自动打开。
    pub fn add_panel<T: Panel>(&mut self, panel: Entity<T>, cx: &mut Context<Self>) {
        use std::sync::Arc;
        let position = panel.read(cx).default_position(cx);
        let starts_open = panel.read(cx).starts_open(cx);
        let panel_handle = Arc::new(panel) as Arc<dyn PanelHandle>;

        let dock_entity = match position {
            DockPosition::Left => &self.left_dock,
            DockPosition::Right => &self.right_dock,
            DockPosition::Bottom => &self.bottom_dock,
        };

        dock_entity.update(cx, |dock, cx| {
            dock.add_panel(panel_handle);
            if starts_open {
                dock.set_open(true);
            }
            cx.notify();
        });
    }

    /// 打开并 focus 指定类型的 Panel（通过 PanelButtons 的 Dock 触发）。
    /// 对齐 zed `Workspace::toggle_panel_focus::<T>`。
    pub fn toggle_panel_focus<T: Panel>(&mut self, cx: &mut Context<Self>) -> bool {
        let (dock_entity, is_open) = self.find_dock_with_panel::<T>(cx);
        let was_open = is_open;

        if let Some(dock_entity) = dock_entity {
            dock_entity.update(cx, |dock, cx| {
                if was_open {
                    dock.set_open(false);
                } else {
                    dock.open_panel::<T>();
                }
                cx.notify();
            });
        }

        !was_open
    }

    /// 让指定类型的 Panel 所在 Dock 打开。
    pub fn open_panel<T: Panel>(&mut self, cx: &mut Context<Self>) {
        if let Some(dock_entity) = self.find_dock_entity::<T>(cx) {
            dock_entity.update(cx, |dock, cx| {
                dock.open_panel::<T>();
                cx.notify();
            });
        }
    }

    /// 让指定类型的 Panel 所在 Dock 关闭。
    pub fn close_panel<T: Panel>(&self, cx: &mut Context<Self>) {
        if let Some(dock_entity) = self.find_dock_entity::<T>(cx) {
            dock_entity.update(cx, |dock, cx| {
                dock.close_panel::<T>();
                cx.notify();
            });
        }
    }

    fn find_dock_entity<T: Panel>(&self, cx: &App) -> Option<&Entity<Dock>> {
        if self.left_dock.read(cx).has_panel::<T>() {
            Some(&self.left_dock)
        } else if self.right_dock.read(cx).has_panel::<T>() {
            Some(&self.right_dock)
        } else if self.bottom_dock.read(cx).has_panel::<T>() {
            Some(&self.bottom_dock)
        } else {
            None
        }
    }

    fn find_dock_with_panel<T: Panel>(&self, cx: &App) -> (Option<&Entity<Dock>>, bool) {
        if let Some(dock) = self.find_dock_entity::<T>(cx) {
            let is_open = dock.read(cx).is_open();
            (Some(dock), is_open)
        } else {
            (None, false)
        }
    }

    /// 遍历所有 Dock，按类型拿到 Entity<T>（对齐 Zed workspace.rs:4825）。
    pub fn panel<T: Panel>(&self, cx: &App) -> Option<Entity<T>> {
        [&self.left_dock, &self.bottom_dock, &self.right_dock]
            .iter()
            .find_map(|dock| dock.read(cx).panel::<T>())
    }

    /// App 层创建 MultiWorkspace 后传给 Workspace，Workspace 传给 StatusBar。
    /// StatusBar toggle sidebar 时通过 MultiWorkspace 中转（不直接碰 Sidebar entity）。
    pub fn set_multi_workspace(
        &mut self,
        mw: Entity<crate::multi_workspace::MultiWorkspace>,
        cx: &mut Context<Self>,
    ) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_multi_workspace(mw.clone());
            cx.notify();
        });
    }

    /// 对齐 zed `resize_left_dock` — 调整左 dock 宽度。
    /// 关键 clamp: 不能把右 dock 挤没（对齐 zed workspace.rs:L8969-8980）。
    fn resize_left_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        let workspace_width = self.bounds.right().as_f32() - self.bounds.left().as_f32();
        let size = new_size.min(workspace_width - RESIZE_HANDLE_SIZE);
        self.left_dock.update(cx, |dock, cx| {
            dock.set_size(size);
            cx.notify();
        });
    }

    /// 对齐 zed `resize_right_dock` — 调整右 dock 宽度。
    /// 关键 clamp: 不能把左 dock 挤没（对齐 zed workspace.rs:L8988-8998）。
    fn resize_right_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        let workspace_width = self.bounds.right().as_f32() - self.bounds.left().as_f32();
        let size = new_size.min(workspace_width - RESIZE_HANDLE_SIZE);
        self.right_dock.update(cx, |dock, cx| {
            dock.set_size(size);
            cx.notify();
        });
    }

    /// 对齐 zed `resize_bottom_dock` — 调整底部 dock 高度。
    /// bounds 现在是 main_area 的 bounds (不含 titlebar/statusbar), 所以
    /// `bounds.bottom() - RESIZE_HANDLE - bounds.top()` 即 `bounds.height - RESIZE_HANDLE`。
    fn resize_bottom_dock(&mut self, new_size: f32, cx: &mut Context<Self>) {
        let size = new_size
            .min(self.bounds.bottom().as_f32() - RESIZE_HANDLE_SIZE - self.bounds.top().as_f32());
        self.bottom_dock.update(cx, |dock, cx| {
            dock.set_size(size);
            cx.notify();
        });
    }

    /// 对齐 zed `render_dock(&self, position, dock, window, cx)` (workspace.rs:L8628-8720)。
    ///
    /// Zed 关键实现：
    /// - 容器默认 `.flex().flex_none()` — 不管开/关都有 flex_none
    /// - Left/Right dock: `.w(size)` + `flex_shrink(1.0)`
    /// - Bottom dock: `.h(size)` 不加 flex_shrink
    fn render_dock(
        &self,
        position: DockPosition,
        dock: &Entity<Dock>,
        cx: &App,
    ) -> impl IntoElement {
        let dock_ref = dock.read(cx);
        let is_open = dock_ref.is_open();
        let size = dock_ref.current_size(cx);

        let is_bottom = matches!(position, DockPosition::Bottom);

        let mut container = div()
            .id(match position {
                DockPosition::Left => "left-dock",
                DockPosition::Right => "right-dock",
                DockPosition::Bottom => "bottom-dock",
            })
            .flex()
            .overflow_hidden()
            .flex_none()
            .child(dock.clone());

        if is_open {
            if is_bottom {
                container = container.h(px(size));
            } else {
                container = container.w(px(size)).flex_shrink(1.0);
            }
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

        let left_dock = self.render_dock(DockPosition::Left, &self.left_dock, cx);
        let right_dock = self.render_dock(DockPosition::Right, &self.right_dock, cx);
        let bottom_dock = self.render_dock(DockPosition::Bottom, &self.bottom_dock, cx);

        let center = self.center.render();

        // 读取 bottom dock 布局
        let bottom_layout = Self::read_bottom_dock_layout(cx);

        // 主区域 — 4 种布局树（逐行对齐 zed workspace.rs:L9748-L9982）
        let main_area = match bottom_layout {
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
                        .overflow_hidden()
                        .child(div().h_flex().flex_1().child(center))
                        .child(bottom_dock),
                )
                .child(right_dock)
                .into_any_element(),

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
                                .child(div().h_flex().flex_1().child(center)),
                        )
                        .child(right_dock),
                )
                .child(bottom_dock)
                .into_any_element(),

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
                                        .child(div().h_flex().flex_1().child(center)),
                                ),
                        )
                        .child(bottom_dock),
                )
                .child(right_dock)
                .into_any_element(),

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
                                        .child(div().h_flex().flex_1().child(center)),
                                )
                                .child(right_dock),
                        )
                        .child(bottom_dock),
                )
                .into_any_element(),
        };

        let mut root = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // zed workspace.rs:L9595 — TitleBar
            .when_some(self.titlebar_item.clone(), |root, item| root.child(item))
            .child(
                div()
                    .size_full()
                    .relative()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("workspace")
                            .bg(colors.background)
                            .relative()
                            .flex_1()
                            .w_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .border_t_1()
                            .border_b_1()
                            .border_color(colors.border)
                            .child({
                                let this = cx.entity();
                                canvas(
                                    move |bounds, _window, cx| {
                                        this.update(cx, |workspace, cx| {
                                            let bounds_changed = workspace.bounds != bounds;
                                            workspace.bounds = bounds;

                                            if bounds_changed {
                                                workspace.left_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.width.as_f32(),
                                                    );
                                                });
                                                workspace.right_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.width.as_f32(),
                                                    );
                                                });
                                                workspace.bottom_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.height.as_f32(),
                                                    );
                                                });
                                            }
                                        })
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full()
                            })
                            // zed workspace.rs:L9707 — on_drag_move 挂在 #workspace
                            .on_drag_move::<DraggedDock>(cx.listener(
                                move |workspace: &mut Self,
                                      e: &DragMoveEvent<DraggedDock>,
                                      _window: &mut Window,
                                      cx| {
                                    if workspace.previous_dock_drag_coordinates
                                        != Some(e.event.position)
                                    {
                                        workspace.previous_dock_drag_coordinates =
                                            Some(e.event.position);
                                        let bounds = workspace.bounds;
                                        match e.drag(cx).0 {
                                            DockPosition::Left => {
                                                workspace.resize_left_dock(
                                                    e.event.position.x.as_f32()
                                                        - bounds.left().as_f32(),
                                                    cx,
                                                );
                                            }
                                            DockPosition::Right => {
                                                workspace.resize_right_dock(
                                                    bounds.right().as_f32()
                                                        - e.event.position.x.as_f32(),
                                                    cx,
                                                );
                                            }
                                            DockPosition::Bottom => {
                                                workspace.resize_bottom_dock(
                                                    bounds.bottom().as_f32()
                                                        - e.event.position.y.as_f32(),
                                                    cx,
                                                );
                                            }
                                        }
                                    }
                                },
                            ))
                            .child(main_area),
                    )
                    // zed workspace.rs:L10009 — status_bar 和 #workspace 同级
                    .child(self.status_bar.clone()),
            );

        // 外部注册的 workspace action — 逐个 chain on_action
        for action_cb in self.workspace_actions.iter() {
            root = action_cb.apply(root, cx);
        }

        root
    }
}
