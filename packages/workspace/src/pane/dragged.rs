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
pub struct DraggedTab {
    /// 来源 Pane
    pub pane: Entity<Pane>,
    /// 被拖的 item（dyn object 持有）
    pub item: Box<dyn ItemHandle>,
    /// tab 在 source pane 里的索引（拖拽前位置）
    pub ix: usize,
    /// 被拖 tab 在 source pane 里是不是当前激活项
    pub is_active: bool,
}

// 手动 Clone — ItemHandle 有 boxed_clone，Box<dyn ItemHandle> 不能 auto Clone
// （对齐 zed crate/workspace/src/pane.rs:524 的 DraggedTab 也有 boxed_clone）
impl std::clone::Clone for DraggedTab {
    fn clone(&self) -> Self {
        Self {
            pane: self.pane.clone(),
            item: self.item.boxed_clone(),
            ix: self.ix,
            is_active: self.is_active,
        }
    }
}

/// 行选择的拖拽标记（编辑器内部用）。
///
/// Zed 在 pane.rs 里也定义了一个 DraggedSelection，
/// AAgent 还没有编辑器，先占位。
#[derive(Clone, Debug)]
pub struct DraggedSelection;

// 以后还会有：
// - SplitDirection { Left, Right, Up, Down }       // pane split 方向
// - SplitMode { ClonePane, EmptyPane, MovePane }    // split 时 item 处理
// - drag_split_direction: Option<SplitDirection>    // Pane 内部标记当前悬停在哪个 split handle
