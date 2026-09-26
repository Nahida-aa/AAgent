//! Pane 相关的拖拽标记类型。
//!
//! 对齐 Zed `crates/workspace/src/pane.rs` 中的 `DraggedTab` / `DraggedSelection` 等。
//! GPUI 的 drag & drop 机制需要这些类型实现 Clone（跨上下文传递）。
//!
//! Zed 版本：
//! ```ignore
//! pub struct DraggedTab {
//!     pub pane: Entity<Pane>,
//!     pub item: Box<dyn ItemHandle>,
//!     pub ix: usize,
//!     pub detail: usize,        // Zed 用来区分 preview vs full open，AAgent 暂不需要
//!     pub is_active: bool,
//! }
//! ```

use gpui::{Entity, prelude::*};

use crate::Pane;
use crate::item::ItemHandle;

/// Tab 被拖拽时携带的数据。
///
/// 当用户按住一个 tab 并拖动时，GPUI 的 drag 框架会 clone 这个值，
/// drop 目标（另一个 Pane 的 tab bar / PaneGroup 的 split handle / Dock）
/// 通过类型匹配拿到它，决定怎么处理（reorder / split / 移动到其他 pane）。
///
/// # Clone
/// `Box<dyn ItemHandle>` 不能自动 Clone（trait object 不知道具体类型），
/// 手动 impl Clone 调用 `ItemHandle::boxed_clone`。
#[derive(Clone)]
pub struct DraggedTab {
    /// 来源 Pane
    pub pane: Entity<Pane>,
    /// 被拖的 item（dyn object 持有）
    pub item: Box<dyn ItemHandle>,
    /// tab 在 source pane 里的索引（拖拽前位置）
    pub ix: usize,
    pub detail: usize,
    /// 被拖 tab 在 source pane 里是不是当前激活项
    pub is_active: bool,
}
