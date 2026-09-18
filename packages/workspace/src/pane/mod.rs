//! Pane — Tab 容器，持有多个 Item + 一个激活项。
//!
//! 对齐 zed `crates/workspace/src/pane.rs` 的核心子集。
//!
//! 子模块：
//! - [event] — Event enum（Empty, ActiveItemChanged, ItemAdded, ItemClosed）
//! - [history] — ActivationHistory，实现 "activate last"
//! - [activate_item] — ActivateItem action（带字段的 action 单独放）
//! - [dragged] — DraggedTab / DraggedSelection drag marker

use gpui::{
    App, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, actions, div, prelude::*, px,
};
use ui_gpui::IconName;
use ui_gpui::theme::ActiveTheme;

use crate::item::{Item, ItemHandle};

pub mod activate_item;
pub mod dragged;
pub mod event;
pub mod history;

pub use activate_item::ActivateItem;
pub use dragged::{DraggedSelection, DraggedTab};
pub use event::Event;
pub use history::ActivationHistory;

actions!(
    pane,
    [
        CloseActiveItem,
        ActivateNextItem,
        ActivatePreviousItem,
        ClosePane,
        /// 激活最近激活过的 item（对齐 zed ActivateLastItem）。
        ActivateLastItem,
    ]
);

/// Tab 容器 — 装多个 Item，顶部 tab bar 切换。
pub struct Pane {
    focus_handle: FocusHandle,
    items: Vec<Box<dyn ItemHandle>>,
    active_item_index: usize,
    close_pane_if_empty: bool,
    /// 激活历史 — 最近激活的 item 排最前，实现 "activate last"。
    activation_history: ActivationHistory,
}

impl Pane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
            active_item_index: 0,
            close_pane_if_empty: true,
            activation_history: ActivationHistory::new(),
        }
    }

    pub fn add_item<T: Item>(&mut self, item: Entity<T>, cx: &mut Context<Self>) {
        let entity_id = item.entity_id();
        self.items.push(Box::new(item) as Box<dyn ItemHandle>);
        self.active_item_index = self.items.len() - 1;
        self.activation_history.record_activation(entity_id);
        cx.emit(Event::ItemAdded(entity_id));
        cx.emit(Event::ActiveItemChanged);
        cx.notify();
    }

    pub fn activate_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.items.len() || self.active_item_index == index {
            return;
        }
        self.active_item_index = index;
        let entity_id = self.items[index].item_id();
        self.activation_history.record_activation(entity_id);
        cx.emit(Event::ActiveItemChanged);
        cx.notify();
    }

    pub fn close_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.items.len() {
            return;
        }
        let closed_id = self.items[index].item_id();
        let _ = self.items.remove(index);
        self.activation_history.remove(closed_id);

        if self.items.is_empty() {
            self.active_item_index = 0;
            if self.close_pane_if_empty {
                cx.emit(Event::Empty);
            }
        } else {
            self.active_item_index = (self.active_item_index.min(self.items.len() - 1)).max(0);
        }
        cx.emit(Event::ItemClosed(closed_id));
        cx.notify();
    }

    /// 激活最近激活过的 item（除了当前 active 的）。
    /// 对齐 Zed 的 ActivateLastItem action。
    pub fn activate_last_item(&mut self, cx: &mut Context<Self>) {
        if self.items.len() < 2 {
            return;
        }
        let current_id = self.items[self.active_item_index].item_id();
        if let Some(recent_id) = self.activation_history.most_recent_excluding(current_id) {
            if let Some(index) = self.items.iter().position(|it| it.item_id() == recent_id) {
                self.activate_item(index, cx);
            }
        }
    }

    pub fn items(&self) -> &[Box<dyn ItemHandle>] {
        &self.items
    }

    pub fn active_item_index(&self) -> usize {
        self.active_item_index
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl EventEmitter<Event> for Pane {}

impl Focusable for Pane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex_col()
            .on_action(cx.listener(|this: &mut Self, _: &CloseActiveItem, _, cx| {
                if !this.items.is_empty() {
                    this.close_item(this.active_item_index, cx);
                }
            }))
            .on_action(cx.listener(|this: &mut Self, e: &ActivateItem, _, cx| {
                this.activate_item(e.0, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &ActivateNextItem, _, cx| {
                if this.items.len() > 1 {
                    let next = (this.active_item_index + 1) % this.items.len();
                    this.activate_item(next, cx);
                }
            }))
            .on_action(
                cx.listener(|this: &mut Self, _: &ActivatePreviousItem, _, cx| {
                    if this.items.len() > 1 {
                        let prev =
                            (this.active_item_index + this.items.len() - 1) % this.items.len();
                        this.activate_item(prev, cx);
                    }
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &ActivateLastItem, _, cx| {
                this.activate_last_item(cx);
            }))
            .child(self.render_tab_bar(cx))
            .child(self.render_active_item(cx))
    }
}

impl Pane {
    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        if self.items.is_empty() {
            return div();
        }

        let mut tab_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .px_1()
            .h(px(28.0))
            .border_b_1()
            .border_color(colors.border);

        for (i, item) in self.items.iter().enumerate() {
            let is_active = i == self.active_item_index;
            let icon = item.tab_icon(cx);
            let label = item.tab_label(cx);

            let tab_id = format!("pane-tab-{i}");
            let close_id = format!("pane-tab-close-{i}");

            tab_bar = tab_bar.child(
                div()
                    .id(tab_id)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .h_full()
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .bg(if is_active {
                        colors.panel_background
                    } else {
                        colors.surface_background
                    })
                    .child(ui_gpui::base::icon::Icon::new(icon).size(px(14.0)))
                    .child(label)
                    .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                        this.activate_item(i, cx);
                    }))
                    .child(
                        div()
                            .id(close_id)
                            .pl_1()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                                this.close_item(i, cx);
                            }))
                            .child(ui_gpui::base::icon::Icon::new(IconName::Close).size(px(12.0))),
                    ),
            );
        }
        tab_bar
    }

    fn render_active_item(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        if let Some(item) = self.items.get(self.active_item_index) {
            item.render_content(cx)
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .text_color(colors.text_muted)
                .child("Pane 为空")
                .into_any_element()
        }
    }
}
