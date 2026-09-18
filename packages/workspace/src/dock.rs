//! Dock 面板容器，对齐 zed `dock.rs`。

use gpui::{
    Context, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Render, Styled,
    Window, deferred, div, hsla, prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;

use crate::dock_position::DockPosition;
use crate::panel::PanelEntry;

/// Resize handle 的大小（对齐 zed dock.rs `RESIZE_HANDLE_SIZE = px(6.)`）。
pub(crate) const RESIZE_HANDLE_SIZE: f32 = 6.0;

/// 拖拽 marker — 标识当前哪个 Dock 正在被 resize。
/// Workspace 顶层 div 的 `on_drag_move` listener 会匹配这个类型。
#[derive(Clone)]
pub(crate) struct DraggedDock(pub DockPosition);

impl Render for DraggedDock {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// 面板容器（对齐 zed `pub struct Dock` dock.rs:L283-L295）。
pub struct Dock {
    position: DockPosition,
    panel_entries: Vec<PanelEntry>,
    is_open: bool,
    active_panel_index: Option<usize>,
    /// 用户调整后的尺寸（覆盖 active panel 的 default_size）。
    /// None = 用 active panel 的 default_size。
    size_override: Option<f32>,
}

impl Dock {
    pub fn new(position: DockPosition) -> Self {
        Self {
            position,
            panel_entries: Vec::new(),
            is_open: false,
            active_panel_index: None,
            size_override: None,
        }
    }

    pub fn position(&self) -> DockPosition {
        self.position
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn active_panel_index(&self) -> Option<usize> {
        self.active_panel_index
    }

    pub fn panel_entries(&self) -> &[PanelEntry] {
        &self.panel_entries
    }

    pub fn add_panel(&mut self, entry: PanelEntry) {
        self.panel_entries.push(entry);
        if self.active_panel_index.is_none() {
            self.active_panel_index = Some(0);
        }
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.is_open = false;
        } else {
            self.is_open = true;
            if self.active_panel_index.is_none() && !self.panel_entries.is_empty() {
                self.active_panel_index = Some(0);
            }
        }
    }

    pub fn activate_panel(&mut self, index: usize) {
        if index < self.panel_entries.len() {
            self.active_panel_index = Some(index);
        }
    }

    /// 移除并返回 index 的面板（用于跨 Dock 移动）。
    pub fn remove_panel(&mut self, index: usize) -> Option<PanelEntry> {
        if index < self.panel_entries.len() {
            let removed = self.panel_entries.remove(index);
            // 修正 active_panel_index
            match self.active_panel_index {
                Some(ai) if ai == index => {
                    self.active_panel_index = self
                        .panel_entries
                        .get(ai)
                        .map(|_| ai)
                        .or(self.panel_entries.first().map(|_| 0));
                }
                Some(ai) if ai > index => {
                    self.active_panel_index = Some(ai - 1);
                }
                _ => {}
            }
            if self.panel_entries.is_empty() {
                self.active_panel_index = None;
            }
            Some(removed)
        } else {
            None
        }
    }

    /// 当前 Dock 的实际尺寸（对齐 zed workspace.rs `dock_size`）。
    /// - Left/Right Dock → width (px)
    /// - Bottom Dock → height (px)
    ///
    /// 优先用 size_override（用户 resize 过），否则用 active panel 的 default_size。
    pub fn current_size(&self) -> f32 {
        if let Some(override_size) = self.size_override {
            return override_size;
        }
        let active = self
            .active_panel_index
            .and_then(|i| self.panel_entries.get(i))
            .map(|e| e.kind);
        match active {
            Some(kind) => {
                let (w, h) = kind.default_size();
                match self.position {
                    DockPosition::Left | DockPosition::Right => w,
                    DockPosition::Bottom => h,
                }
            }
            None => 280.0, // fallback
        }
    }

    /// 用户 resize 后设置新尺寸（对齐 zed `set_dock_size`）。
    pub fn set_size(&mut self, size: f32) {
        // 限制范围：最小 80px，最大窗口一半（简化）
        let clamped = size.clamp(RESIZE_HANDLE_SIZE + 74.0, 1200.0);
        self.size_override = Some(clamped);
    }

    /// 重置为 active panel 的 default_size（用户调乱了想恢复）。
    pub fn reset_size(&mut self) {
        self.size_override = None;
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.is_open || self.panel_entries.is_empty() {
            return div().into_any_element();
        }

        let colors = cx.theme().colors();
        let position = self.position;

        // Active panel content — 占位（后续换成真实 Panel entity 渲染）
        // Zed Dock Render 没有 tab bar —— 激活的 Panel 自己渲染自己的 UI。
        let active_kind = self
            .active_panel_index
            .and_then(|i| self.panel_entries.get(i))
            .map(|e| e.kind);

        let content = match active_kind {
            Some(kind) => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h_full()
                .text_size(px(16.0))
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child(format!("{} (placeholder)", kind.aria_label())),
            None => div(),
        };

        // Resize handle — 对齐 zed dock.rs `create_resize_handle()`。
        // 用 deferred + absolute 定位在 Dock 边缘，一半在 dock 内一半在 dock 外。
        let resize_handle = {
            let pos = self.position;
            let handle = div()
                .id(format!("dock-resize-handle-{:?}", pos))
                .on_drag(DraggedDock(pos), |dock, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| dock.clone())
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|dock: &mut Self, e: &MouseUpEvent, _, cx| {
                        if e.click_count == 2 {
                            dock.reset_size();
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }),
                )
                .occlude();

            match pos {
                DockPosition::Left => deferred(
                    handle
                        .absolute()
                        .right(px(-RESIZE_HANDLE_SIZE / 2.))
                        .top(px(0.))
                        .h_full()
                        .w(px(RESIZE_HANDLE_SIZE))
                        .cursor_col_resize(),
                ),
                DockPosition::Right => deferred(
                    handle
                        .absolute()
                        .top(px(0.))
                        .left(px(-RESIZE_HANDLE_SIZE / 2.))
                        .h_full()
                        .w(px(RESIZE_HANDLE_SIZE))
                        .cursor_col_resize(),
                ),
                DockPosition::Bottom => deferred(
                    handle
                        .absolute()
                        .top(px(-RESIZE_HANDLE_SIZE / 2.))
                        .left(px(0.))
                        .w_full()
                        .h(px(RESIZE_HANDLE_SIZE))
                        .cursor_row_resize(),
                ),
            }
        };

        let root = div()
            .id("dock-panel")
            .relative()
            .w_full()
            .h_full()
            .bg(colors.panel_background)
            .border_color(colors.border)
            .overflow_hidden()
            .map(|el| match position.axis() {
                gpui::Axis::Horizontal => el.flex_col(),
                gpui::Axis::Vertical => el.flex_row(),
            })
            .map(|el| match position {
                DockPosition::Left => el.border_r_1(),
                DockPosition::Right => el.border_l_1(),
                DockPosition::Bottom => el.border_t_1(),
            })
            .child(content)
            .child(resize_handle);

        root.into_any_element()
    }
}
