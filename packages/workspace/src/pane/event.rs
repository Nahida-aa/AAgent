//! Pane 事件类型。
//!
//! Zed 在 `pane.rs` 里直接定义 Event enum，AAgent 单独放这个模块
//! 方便后续扩展（比如加 Zoom、Split 相关事件）。

use gpui::EntityId;

/// Pane 内部事件。Pane 通过 `EventEmitter<Event>` 发出，
/// Workspace 或 Panel 订阅来响应（比如 Empty 时关闭面板）。
#[derive(Debug, Clone)]
pub enum Event {
    /// 所有 item 被关闭，Pane 变空。
    Empty,
    /// active item 变了（用户点 tab、activate_item、close active 都会触发）。
    ActiveItemChanged,
    /// 某个 item 被关闭。携带被关闭 item 的 EntityId。
    ItemClosed(EntityId),
    /// 某个 item 被添加。携带新 item 的 EntityId。
    ItemAdded(EntityId),
}
