//! Pane 事件类型。
//!
//! Zed 在 `pane.rs` 里直接定义 Event enum，AAgent 单独放这个模块
//! 方便后续扩展（比如加 Zoom、Split 相关事件）。

use gpui::EntityId;

pub enum Event {
    AddItem {
        item: Box<dyn ItemHandle>,
    },
    ActivateItem {
        local: bool,
        focus_changed: bool,
    },
    Remove {
        focus_on_pane: Option<Entity<Pane>>,
    },
    RemovedItem {
        item: Box<dyn ItemHandle>,
    },
    Split {
        direction: SplitDirection,
        mode: SplitMode,
    },
    ItemPinned,
    ItemUnpinned,
    JoinAll,
    JoinIntoNext,
    ChangeItemTitle,
    Focus,
    ZoomIn,
    ZoomOut,
    UserSavedItem {
        item: Box<dyn WeakItemHandle>,
        save_intent: SaveIntent,
    },
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::AddItem { item } => f
                .debug_struct("AddItem")
                .field("item", &item.item_id())
                .finish(),
            Event::ActivateItem { local, .. } => f
                .debug_struct("ActivateItem")
                .field("local", local)
                .finish(),
            Event::Remove { .. } => f.write_str("Remove"),
            Event::RemovedItem { item } => f
                .debug_struct("RemovedItem")
                .field("item", &item.item_id())
                .finish(),
            Event::Split { direction, mode } => f
                .debug_struct("Split")
                .field("direction", direction)
                .field("mode", mode)
                .finish(),
            Event::JoinAll => f.write_str("JoinAll"),
            Event::JoinIntoNext => f.write_str("JoinIntoNext"),
            Event::ChangeItemTitle => f.write_str("ChangeItemTitle"),
            Event::Focus => f.write_str("Focus"),
            Event::ZoomIn => f.write_str("ZoomIn"),
            Event::ZoomOut => f.write_str("ZoomOut"),
            Event::UserSavedItem { item, save_intent } => f
                .debug_struct("UserSavedItem")
                .field("item", &item.id())
                .field("save_intent", save_intent)
                .finish(),
            Event::ItemPinned => f.write_str("ItemPinned"),
            Event::ItemUnpinned => f.write_str("ItemUnpinned"),
        }
    }
}
