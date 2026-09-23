//! Toolbar — 编辑器/面板顶部的工具条容器。
//!
//! 对齐 Zed `crates/workspace/src/toolbar.rs`。
//! 简化版：去掉 `PaneSearchBarCallbacks`（Zed workspace 全局回调，需 language registry）、
//! 去掉 `can_navigate`，用 `px(...)` 代替 Zed 的 `DynamicSpacing`。

use crate::ItemHandle;
use ui::{h_flex, prelude::*, v_flex};
use gpui::{
    AnyView, App, Context, Div, Entity, EntityId, EventEmitter, Global, KeyContext,
    ParentElement as _, Render, Styled, Window,
};
use language::LanguageRegistry;
use std::sync::Arc;

pub struct PaneSearchBarCallbacks {
    pub setup_search_bar:
        fn(Option<Arc<LanguageRegistry>>, &Entity<Toolbar>, &mut Window, &mut App),
    pub wrap_div_with_search_actions: fn(Div, Entity<crate::Pane>) -> Div,
}

impl Global for PaneSearchBarCallbacks {}

/// Toolbar item 向 Toolbar 发出的事件。
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ToolbarItemEvent {
    ChangeLocation(ToolbarItemLocation),
}

/// Toolbar item 在 Toolbar 里的位置。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ToolbarItemLocation {
    Hidden,
    PrimaryLeft,
    PrimaryRight,
    Secondary,
}

/// Toolbar 里每个 item 的 View trait。
///
/// 实现者是 GPUI Entity，Toolbar 通过 `add_item` 收集后一起 render。
pub trait ToolbarItemView: Render + EventEmitter<ToolbarItemEvent> {
    /// 当 active pane item 变化时调用，返回新的 location。
    fn set_active_pane_item(
        &mut self,
        active_pane_item: Option<&dyn ItemHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ToolbarItemLocation;

    /// Pane 聚焦状态变化时调用（默认空实现）。
    fn pane_focus_update(
        &mut self,
        _pane_focused: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// 向 KeyContext 贡献额外 context（默认空实现）。
    fn contribute_context(&self, _context: &mut KeyContext, _cx: &App) {}
}

// ---------- ToolbarItemViewHandle (internal dyn trait) ----------

/// 把 `Entity<T: ToolbarItemView>` 包装成统一接口 — 让 Toolbar 能存 heterogeneous items。
trait ToolbarItemViewHandle: Send {
    fn id(&self) -> EntityId;
    fn to_any(&self) -> AnyView;
    fn set_active_pane_item(
        &self,
        active_pane_item: Option<&dyn ItemHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> ToolbarItemLocation;
    fn focus_changed(&mut self, pane_focused: bool, window: &mut Window, cx: &mut App);
    fn contribute_context(&self, context: &mut KeyContext, cx: &App);
}

// ---------- Toolbar ----------

/// 顶部工具条容器 — 管理一组 ToolbarItemView，按 location 分组渲染。
pub struct Toolbar {
    active_item: Option<Box<dyn ItemHandle>>,
    hidden: bool,
    can_navigate: bool,
    items: Vec<(Box<dyn ToolbarItemViewHandle>, ToolbarItemLocation)>,
}

impl Toolbar {
    fn has_any_visible_items(&self) -> bool {
        self.items
            .iter()
            .any(|(_item, location)| *location != ToolbarItemLocation::Hidden)
    }

    fn left_items(&self) -> impl Iterator<Item = &dyn ToolbarItemViewHandle> {
        self.items.iter().filter_map(|(item, location)| {
            if *location == ToolbarItemLocation::PrimaryLeft {
                Some(item.as_ref())
            } else {
                None
            }
        })
    }

    fn right_items(&self) -> impl Iterator<Item = &dyn ToolbarItemViewHandle> {
        self.items.iter().filter_map(|(item, location)| {
            if *location == ToolbarItemLocation::PrimaryRight {
                Some(item.as_ref())
            } else {
                None
            }
        })
    }

    fn secondary_items(&self) -> impl Iterator<Item = &dyn ToolbarItemViewHandle> {
        self.items.iter().rev().filter_map(|(item, location)| {
            if *location == ToolbarItemLocation::Secondary {
                Some(item.as_ref())
            } else {
                None
            }
        })
    }
}

impl Default for Toolbar {
    fn default() -> Self { Self::new() }
}
impl ToolBar {
    pub fn new() -> Self {
        Self {
            active_item: None,
            hidden: false,
            items: Default::default(),
            can_navigate: true,
        }
    }
    pub fn set_can_navigate(&mut self, can_navigate: bool, cx: &mut Context<Self>) {
        self.can_navigate = can_navigate;
        cx.notify();
    }
    /// 注册一个 ToolbarItemView。
    ///
    /// Toolbar 会订阅该 Entity 的 ToolbarItemEvent，处理 ChangeLocation。
    pub fn add_item<T>(&mut self, item: Entity<T>, window: &mut Window, cx: &mut Context<Self>)
    where
        T: 'static + ToolbarItemView,
    {
        let location = item.set_active_pane_item(self.active_item.as_deref(), window, cx);
        cx.subscribe(&item, |this, item, event, cx| {
            if let Some((_, current_location)) = this
                .items
                .iter_mut()
                .find(|(i, _)| i.id() == item.entity_id())
            {
                match event {
                    ToolbarItemEvent::ChangeLocation(new_location) => {
                        if new_location != current_location {
                            *current_location = *new_location;
                            cx.notify();
                        }
                    }
                }
            }
        })
        .detach();
        self.items.push((Box::new(item), location));
        cx.notify();
    }
    /// 当 Pane 的 active item 变化时调用。
    ///
    /// 更新 hidden 状态 + 通知所有 toolbar items。
    pub fn set_active_item(
        &mut self,
        item: Option<&dyn ItemHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_item = item.map(|item| item.boxed_clone());
        self.hidden = self
            .active_item
            .as_ref()
            .map(|item| !item.show_toolbar(cx))
            .unwrap_or(false);

        for (toolbar_item, current_location) in self.items.iter_mut() {
            let new_location = toolbar_item.set_active_pane_item(item, window, cx);
            if new_location != *current_location {
                *current_location = new_location;
                cx.notify();
            }
        }
    }

    /// Pane 聚焦状态变化 → 透传给所有 items。
    pub fn focus_changed(&mut self, focused: bool, window: &mut Window, cx: &mut Context<Self>) {
        for (toolbar_item, _) in self.items.iter_mut() {
            toolbar_item.focus_changed(focused, window, cx);
        }
    }

    /// 按类型查找已注册的 item。
    pub fn item_of_type<T: ToolbarItemView>(&self) -> Option<Entity<T>> {
        self.items
            .iter()
            .find_map(|(item, _)| item.to_any().downcast().ok())
    }
    pub fn hidden(&self) -> bool { self.hidden }

    /// 向 KeyContext 贡献所有可见 item 的 context。
    pub fn contribute_context(&self, context: &mut KeyContext, cx: &App) {
        for (item, location) in &self.items {
            if *location != ToolbarItemLocation::Hidden {
                item.contribute_context(context, cx);
            }
        }
    }
}

impl<T: ToolbarItemView> ToolbarItemViewHandle for Entity<T> {
    fn id(&self) -> EntityId { self.entity_id() }

    fn to_any(&self) -> AnyView { self.clone().into() }

    fn set_active_pane_item(
        &self,
        active_pane_item: Option<&dyn ItemHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> ToolbarItemLocation {
        self.update(cx, |this, cx| {
            this.set_active_pane_item(active_pane_item, window, cx)
        })
    }

    fn focus_changed(&mut self, pane_focused: bool, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| {
            this.pane_focus_update(pane_focused, window, cx);
            cx.notify();
        });
    }

    fn contribute_context(&self, context: &mut KeyContext, cx: &App) {
        self.read(cx).contribute_context(context, cx)
    }
}

// ---------- Render ----------

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.hidden || !self.has_any_visible_items() {
            return div();
        }

        let secondary_items = self.secondary_items().map(|item| item.to_any());

        let has_left_items = self.left_items().count() > 0;
        let has_right_items = self.right_items().count() > 0;

        v_flex()
            .relative()
            .py(px(4.))
            .px(px(8.))
            .when(has_left_items || has_right_items, |this| this.gap(px(4.)))
            .border_b_1()
            .child(
                h_flex()
                    .items_start()
                    .justify_between()
                    .gap(px(8.))
                    .when(has_left_items, |this| {
                        this.child(
                            h_flex()
                                .min_h(px(32.))
                                .flex_1()
                                .justify_start()
                                .overflow_x_hidden()
                                .children(self.left_items().map(|item| item.to_any())),
                        )
                    })
                    .when(has_right_items, |this| {
                        this.child(
                            h_flex()
                                .h(px(32.))
                                .flex_row_reverse()
                                .when(has_left_items, |this| this.flex_none())
                                .justify_end()
                                .children(self.right_items().map(|item| item.to_any())),
                        )
                    }),
            )
            .children(secondary_items)
    }
}
