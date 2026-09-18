//! ItemHandle trait — Pane 里装的东西统一接口。
//!
//! 对齐 Zed `WeakItemHandle` + `ItemHandle` 的核心子集。
//!
//! Zed:
//! - `WeakItemHandle: Send + Sync` — 弱引用，upgrade 回 ItemHandle
//! - `ItemHandle` — 强引用 trait，~20 方法（telemetry/save/preview/find 等）
//!
//! AAgent:
//! - `WeakItemHandle` — 用 `WeakEntity<T>` 实现，不加 Send/Sync（GPUI 主线程）
//! - `ItemHandle` — 简化为 tab + render 核心方法
//! - `Item` — 强类型小 trait，Entity<T> 自动获得 ItemHandle + downgrade

use gpui::{AnyElement, App, Entity, EntityId, FocusHandle, IntoElement, SharedString, WeakEntity};
use ui_gpui::IconName;

/// 弱引用 item — 对齐 Zed `WeakItemHandle`。
///
/// Zed: `Arc<Mutex<Option<Entity<Item>>>> + Send + Sync`
/// AAgent: 用 GPUI 原生 `WeakEntity<T>` 实现，不加 Send/Sync（主线程）。
/// upgrade 不需要 cx — GPUI 的 WeakEntity 用原子引用计数，不碰 context。
pub trait WeakItemHandle {
    fn id(&self) -> EntityId;
    fn boxed_clone(&self) -> Box<dyn WeakItemHandle>;
    /// 尝试 upgrade 回强引用 ItemHandle。item 被销毁时返回 None。
    fn upgrade(&self) -> Option<Box<dyn ItemHandle>>;
}

/// Pane 里装的 item 的统一接口（dyn object）。
///
/// 对齐 Zed `ItemHandle` trait，AAgent 简化为 tab + render + downgrade。
pub trait ItemHandle {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
    fn item_id(&self) -> EntityId;
    fn item_focus_handle(&self, cx: &App) -> FocusHandle;
    fn render_content(&self, cx: &App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
    /// 降级为弱引用 — 对齐 Zed `ItemHandle::downgrade_item`。
    fn downgrade_item(&self) -> Box<dyn WeakItemHandle>;
}

/// 每种 item 类型实现此小 trait，Entity<T> 自动获得 ItemHandle + WeakItemHandle。
pub trait Item: 'static + gpui::Render + gpui::Focusable {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
}

// ─── WeakEntity<T> → WeakItemHandle ──────────────────────────────────────

impl<T: Item> WeakItemHandle for WeakEntity<T> {
    fn id(&self) -> EntityId {
        self.entity_id()
    }

    fn boxed_clone(&self) -> Box<dyn WeakItemHandle> {
        Box::new(self.clone())
    }

    fn upgrade(&self) -> Option<Box<dyn ItemHandle>> {
        self.upgrade().map(|e| Box::new(e) as Box<dyn ItemHandle>)
    }
}

// ─── Entity<T> → ItemHandle ──────────────────────────────────────────────

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
    fn downgrade_item(&self) -> Box<dyn WeakItemHandle> {
        Box::new(self.downgrade())
    }
}
