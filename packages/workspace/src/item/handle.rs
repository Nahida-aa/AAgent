//! ItemHandle trait — Pane 里装的东西统一接口。
//!
//! 对齐 Zed `crates/workspace/src/item.rs`。

use aa_gpui_kit_ui::IconName;
use gpui::{
    AnyElement, AnyView, App, Entity, EntityId, EventEmitter, FocusHandle, Font, IntoElement,
    SharedString, Subscription, WeakEntity,
};
use language::HighlightedText;

use crate::{ItemEvent, ToolbarItemLocation};

// ─── WeakItemHandle ─────────────────────────────────────────────────────

/// 弱引用 item — 对齐 Zed `WeakItemHandle: Send + Sync`。
pub trait WeakItemHandle: Send + Sync {
    fn id(&self) -> EntityId;
    fn boxed_clone(&self) -> Box<dyn WeakItemHandle>;
    /// 尝试 upgrade 回强引用 ItemHandle。item 被销毁时返回 None。
    fn upgrade(&self) -> Option<Box<dyn ItemHandle>>;
}

// ─── ItemHandle ────────────────────────────────────────────────────────

/// Pane 里装的 item 的统一接口（dyn object）。
///
/// 对齐 Zed `ItemHandle` trait。
pub trait ItemHandle {
    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;
    fn item_id(&self) -> EntityId;
    fn item_focus_handle(&self, cx: &App) -> FocusHandle;
    fn render_content(&self, cx: &App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
    /// 降级为弱引用 — 对齐 Zed `ItemHandle::downgrade_item`。
    fn downgrade_item(&self) -> Box<dyn WeakItemHandle>;

    // ── Toolbar / Breadcrumbs ──

    /// 当前 item 是否要显示 Toolbar（默认 true）。
    fn show_toolbar(&self, _cx: &App) -> bool {
        true
    }

    /// Breadcrumbs 在 Toolbar 的位置（默认 Hidden — 只有实现了 breadcrumbs 的 item 才显示）。
    fn breadcrumb_location(&self, _cx: &App) -> ToolbarItemLocation {
        ToolbarItemLocation::Hidden
    }

    /// 返回 breadcrumbs 分段文字 + 可选字体。
    fn breadcrumbs(&self, _cx: &App) -> Option<(Vec<HighlightedText>, Option<Font>)> {
        None
    }

    /// Breadcrumbs 左边的可选前缀元素（如 git 分支图标）。
    fn breadcrumb_prefix(&self, _window: &mut gpui::Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }

    /// 订阅 item 发出的 ItemEvent。
    fn subscribe_to_item_events(
        &self,
        window: &mut gpui::Window,
        cx: &mut App,
        handler: Box<dyn Fn(ItemEvent, &mut gpui::Window, &mut App)>,
    ) -> Subscription;
}

// ─── Item (强类型 trait) ────────────────────────────────────────────────

/// 每种 item 类型实现此 trait，Entity<T> 自动获得 ItemHandle + WeakItemHandle。
///
/// 完全对齐 Zed `Item: Focusable + EventEmitter<Self::Event> + Render + Sized`。
/// Self::Event 是 item 自定义事件类型（如 EditorEvent、TerminalEvent），
/// 通过 `to_item_events` 桥接成通用的 `ItemEvent`。
pub trait Item:
    'static + gpui::Render + gpui::Focusable + EventEmitter<Self::Event> + Sized
{
    /// item 自定义事件类型（如 `editor::EditorEvent`、`terminal::TerminalEvent`）。
    type Event;

    fn tab_label(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> IconName;

    // ── 事件桥接 ──

    /// 把自定义 `Self::Event` 转换成通用 `ItemEvent`。
    /// 默认空实现（不产生任何 ItemEvent）。
    ///
    /// 对齐 Zed `Item::to_item_events` (item.rs:214)。
    fn to_item_events(_event: &Self::Event, _f: &mut dyn FnMut(ItemEvent)) {}

    // ── Toolbar / Breadcrumbs 默认实现 ──

    /// 当前 item 是否要显示 Toolbar（默认 true）。
    fn show_toolbar(&self, _cx: &App) -> bool {
        true
    }

    /// Breadcrumbs 在 Toolbar 的位置（默认 Hidden）。
    fn breadcrumb_location(&self, _cx: &App) -> ToolbarItemLocation {
        ToolbarItemLocation::Hidden
    }

    /// 返回 breadcrumbs 分段文字 + 可选字体。默认 None（不显示 breadcrumbs）。
    fn breadcrumbs(&self, _cx: &App) -> Option<(Vec<HighlightedText>, Option<Font>)> {
        None
    }

    /// Breadcrumbs 左边的可选前缀元素（如 git 分支图标）。
    fn breadcrumb_prefix(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
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

    fn breadcrumb_location(&self, cx: &App) -> ToolbarItemLocation {
        self.read(cx).breadcrumb_location(cx)
    }
    fn breadcrumbs(&self, cx: &App) -> Option<(Vec<HighlightedText>, Option<Font>)> {
        self.read(cx).breadcrumbs(cx)
    }
    fn breadcrumb_prefix(&self, window: &mut gpui::Window, cx: &mut App) -> Option<AnyElement> {
        self.update(cx, |item, cx| item.breadcrumb_prefix(window, cx))
    }

    fn subscribe_to_item_events(
        &self,
        window: &mut gpui::Window,
        cx: &mut App,
        handler: Box<dyn Fn(ItemEvent, &mut gpui::Window, &mut App)>,
    ) -> Subscription {
        window.subscribe(self, cx, move |_, event, window, cx| {
            T::to_item_events(event, &mut |item_event| handler(item_event, window, cx));
        })
    }
}
