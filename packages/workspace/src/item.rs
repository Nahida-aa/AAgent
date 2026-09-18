//! ItemHandle trait — Pane 里装的东西统一接口。
//!
//! Zed 在 `crates/workspace/src/item.rs` 定义了 ~50 个方法的大 trait，
//! 还要 Send bound + 支持 project/save/reload/telemetry 等。
//! AAgent 简化版只做 terminal 一种 item，trait 最小化。

use gpui::{
    AnyElement, App, Entity, EntityId, FocusHandle, Focusable, IntoElement, Render, SharedString,
};
use ui_gpui::IconName;

/// Pane 里装的 item 的统一接口（dyn object）。
pub trait ItemHandle {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
    fn item_id(&self) -> EntityId;
    fn item_focus_handle(&self, cx: &App) -> FocusHandle;
    fn render_content(&self, cx: &App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
}

/// 每种 item 类型实现此小 trait，Entity<T> 自动获得 ItemHandle。
pub trait Item: 'static + Render + Focusable {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
}

impl<T: Item> ItemHandle for Entity<T> {
    fn tab_label(&self, cx: &App) -> SharedString {
        self.read(cx).tab_label(cx)
    }
    fn tab_icon(&self, cx: &App) -> IconName {
        self.read(cx).tab_icon(cx)
    }
    fn item_id(&self) -> EntityId {
        self.entity_id()
    }
    fn item_focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }
    fn render_content(&self, cx: &App) -> AnyElement {
        self.clone().into_any_element()
    }
    fn boxed_clone(&self) -> Box<dyn ItemHandle> {
        Box::new(self.clone())
    }
}
