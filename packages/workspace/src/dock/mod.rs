//! Dock 面板容器，对齐 zed `dock.rs`。

pub mod panel;
pub mod panel_buttons;

use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement,
    Render, Styled, Window, deferred, div, hsla, prelude::*, px,
};
use settings_content::DockPosition;
use ui_gpui::theme::ActiveTheme;

use self::panel::{Panel, PanelHandle};

/// Resize handle 的大小（对齐 zed dock.rs `RESIZE_HANDLE_SIZE = px(6.)`）。
pub const RESIZE_HANDLE_SIZE: f32 = 6.0;

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
///
/// Zed: `panels: Vec<PanelEntry>` 其中 `PanelEntry { panel: Arc<dyn PanelHandle>, ... }`
/// AAgent: `panels: Vec<Arc<dyn PanelHandle>>` — 简化，PanelSizeState 后续加
pub struct Dock {
    position: DockPosition,
    panels: Vec<Arc<dyn PanelHandle>>,
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
            panels: Vec::new(),
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

    pub fn panels(&self) -> &[Arc<dyn PanelHandle>] {
        &self.panels
    }

    pub fn add_panel(&mut self, panel: Arc<dyn PanelHandle>) {
        self.panels.push(panel);
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
            if self.active_panel_index.is_none() && !self.panels.is_empty() {
                self.active_panel_index = Some(0);
            }
        }
    }

    pub fn activate_panel(&mut self, index: usize) {
        if index < self.panels.len() {
            self.active_panel_index = Some(index);
        }
    }

    /// 移除并返回 index 的面板（用于跨 Dock 移动）。
    pub fn remove_panel(&mut self, index: usize) -> Option<Arc<dyn PanelHandle>> {
        if index < self.panels.len() {
            let removed = self.panels.remove(index);
            match self.active_panel_index {
                Some(ai) if ai == index => {
                    self.active_panel_index = self.panels.first().map(|_| 0);
                }
                Some(ai) if ai > index => {
                    self.active_panel_index = Some(ai - 1);
                }
                _ => {}
            }
            if self.panels.is_empty() {
                self.active_panel_index = None;
            }
            Some(removed)
        } else {
            None
        }
    }

    /// 当前 Dock 的实际尺寸（对齐 zed workspace.rs `dock_size`）。
    pub fn current_size(&self, cx: &App) -> f32 {
        if let Some(override_size) = self.size_override {
            return override_size;
        }
        let active = self.active_panel_index.and_then(|i| self.panels.get(i));
        match active {
            Some(panel) => panel.default_size(cx).as_f32(),
            None => 280.0, // fallback
        }
    }

    /// 是否有用户设置的固定尺寸覆盖（有则不能 flexible sizing）。
    pub fn has_size_override(&self) -> bool {
        self.size_override.is_some()
    }

    /// 用户 resize 后设置新尺寸（对齐 zed dock.rs `resize_active_panel`）。
    /// Zed: `size.max(RESIZE_HANDLE_SIZE).round()` — 只 clamp 最小值, 不设上限
    /// 上限由 Workspace::resize_*_dock 的 clamp_panel_size 处理。
    pub fn set_size(&mut self, size: f32) {
        let clamped = size.max(RESIZE_HANDLE_SIZE + 74.0);
        self.size_override = Some(clamped);
    }

    /// 重置为 active panel 的 default_size。
    pub fn reset_size(&mut self) {
        self.size_override = None;
    }

    /// 对齐 zed `Dock::clamp_panel_size` — 窗口 resize 时, 如果 dock size > main_area 可用空间,
    /// 把 size_override 夹到 max_size。
    /// Zed: clamp_panel_size(max_size, window, cx) → size > max_size 时夹到 max_size.max(RESIZE_HANDLE)
    pub fn clamp_panel_size(&mut self, max_size: f32) {
        let max_size = (max_size - RESIZE_HANDLE_SIZE).abs();
        if let Some(size) = self.size_override {
            if size > max_size {
                self.size_override = Some(max_size.max(RESIZE_HANDLE_SIZE));
            }
        }
    }

    // ---------- 泛型查找（对齐 Zed dock.rs `panel_index_for_type` / `panel::<T>`）----------

    /// 按 Panel 类型查找 index — 用 T::panel_key() 和 PanelHandle::panel_key() 比对。
    pub fn panel_index_for_type<T: Panel>(&self) -> Option<usize> {
        let target = T::panel_key();
        self.panels.iter().position(|p| p.panel_key() == target)
    }

    /// 按 Panel 类型拿到 Entity<T>（Zed dock.rs:521）。
    ///
    /// 实现方式：先按 panel_key 找到 index，再从 Arc<dyn PanelHandle> downcast 成 Entity<T>。
    /// PanelHandle::as_any() 返回 &dyn Any，我们用 Any::downcast_ref 拿回 Entity<T>。
    pub fn panel<T: Panel>(&self) -> Option<Entity<T>> {
        let target = T::panel_key();
        self.panels
            .iter()
            .find(|p| p.panel_key() == target)
            .and_then(|p| {
                let any: &dyn std::any::Any = p.as_any();
                any.downcast_ref::<Entity<T>>().cloned()
            })
    }

    /// Dock 里有没有这个类型的 panel。
    pub fn has_panel<T: Panel>(&self) -> bool {
        self.panel_index_for_type::<T>().is_some()
    }

    /// 让 Dock 打开并激活指定类型的 panel。
    pub fn open_panel<T: Panel>(&mut self) {
        if let Some(index) = self.panel_index_for_type::<T>() {
            self.is_open = true;
            self.active_panel_index = Some(index);
        }
    }

    /// 让 Dock 关闭（如果里面有这个类型的 panel）。
    pub fn close_panel<T: Panel>(&mut self) {
        if self.panel_index_for_type::<T>().is_some() {
            self.is_open = false;
        }
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.is_open || self.panels.is_empty() {
            return div().id("dock-panel").into_any_element();
        }

        let colors = cx.theme().colors();
        let position = self.position;

        // 渲染 active panel 的 entity — zed dock.rs:L1360-1365
        let content = self
            .active_panel_index
            .and_then(|i| self.panels.get(i))
            .map(|p| p.to_any().into_any_element())
            .unwrap_or_else(|| div().into_any_element());

        // Resize handle — 对齐 zed dock.rs:L1275-1331
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

            // Zed dock.rs:L1303-1331 — handle 绝对定位在 dock 的对侧边缘
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

        // Zed dock.rs:L1334-1369 — 关键差异:
        // 1. dock-panel 没有 .relative()（resize handle 用 absolute 相对于最近的 positioned ancestor,
        //    这里 Dock 的 parent (workspace render_dock 的容器) 也是 relative? 不对...
        //    实际上 Dock render 内部没有 relative, 但 resize handle 的 absolute 会相对于祖先的 relative。
        //    我们的容器也没有 relative, 那 resize handle 的 absolute 可能相对于 body... 这是个问题。
        //    等等让我再看 Zed — Zed 的 dock-panel 也没有 relative, 但 resize handle 是 deferred。
        //    实际上 GPUI 的 absolute 是相对于最近的 position!=static 的祖先。
        //    #workspace 是 relative, main_area 的 div 没有 relative, 那 dock-panel 的 absolute handle
        //    会相对于 #workspace 定位? 不对, 应该有一个 relative 的定位上下文。
        //    哦等等 — 我们之前在 Dock render 里加了 .relative(), Zed 没有。但 Zed 的 resize handle 也是
        //    绝对定位的。让我再看看... Zed 的 deferred 是什么意思? GPUI 的 deferred 就是延迟渲染,
        //    absolute 定位一样会找祖先 relative。也许 Zed 的 dock-panel 虽然没显式 .relative(),
        //    但 div 默认就是 position: relative? 让我假设我们需要保留 .relative()。
        //
        // 2. flex 方向按 axis: Left/Right=Horizontal→flex_row, Bottom=Vertical→flex_col
        //    我们之前完全搞反了!
        //
        // 3. panel content 有中间 div 包裹 (w_full.h_full) — zed dock.rs:L1355-1366

        let dock_axis_is_horizontal = matches!(position, DockPosition::Left | DockPosition::Right);

        let root = div()
            .id("dock-panel")
            .relative() // 保留: resize handle absolute 需要定位上下文
            .flex()
            .bg(colors.panel_background)
            .border_color(colors.border)
            .overflow_hidden()
            .map(|el| {
                if dock_axis_is_horizontal {
                    // Left/Right dock: 高度可变 (flex_row 让 panel content + handle 横向?
                    // 不对, handle 是 absolute 的, 所以 flex 方向只影响中间的 content div)
                    el.w_full().h_full().flex_row()
                } else {
                    // Bottom dock
                    el.w_full().h_full().flex_col()
                }
            })
            .map(|el| match position {
                DockPosition::Left => el.border_r_1(),
                DockPosition::Right => el.border_l_1(),
                DockPosition::Bottom => el.border_t_1(),
            })
            // Zed dock.rs:L1354-1366 — 中间 div 包裹 content, 占满 dock-panel
            .child(
                div()
                    .map(|el| {
                        if dock_axis_is_horizontal {
                            el.w_full().h_full()
                        } else {
                            el.w_full().h_full()
                        }
                    })
                    .child(content),
            )
            .child(resize_handle);

        root.into_any_element()
    }
}
