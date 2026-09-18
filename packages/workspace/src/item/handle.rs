//! ItemHandle trait — Pane 里装的东西统一接口。
//!
//! 对齐 Zed `WeakItemHandle` + `ItemHandle` 的核心子集。
//!
//! Zed 有两个 trait:
//! - `WeakItemHandle` — Arc<Mutex<Option<Entity<Item>>>>，可 downgrade/upgrade
//! - `ItemHandle` 里还有 ~20 个方法（telemetry/save/preview/find 等）
//!
//! AAgent 简化:
//! - 没有 WeakItemHandle（用 EntityId 直接 downgrade/upgrade）
//! - ItemHandle 只保留 Pane 切 tab 和渲染必需的方法

use gpui::{AnyElement, App, Entity, EntityId, FocusHandle, IntoElement, SharedString};
use ui_gpui::IconName;

/// Pane 里装的 item 的统一接口（dyn object）。
///
/// 对齐 Zed `ItemHandle` trait，AAgent 简化为 tab + render 核心方法。
pub trait ItemHandle {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
    fn item_id(&self) -> EntityId;
    fn item_focus_handle(&self, cx: &App) -> FocusHandle;
    fn render_content(&self, cx: &App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
}

/// 每种 item 类型实现此小 trait，Entity<T> 自动获得 ItemHandle。
pub trait Item: 'static + gpui::Render + gpui::Focusable {
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
