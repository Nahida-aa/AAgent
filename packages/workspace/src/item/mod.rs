//! Item — Pane 里装的东西的统一接口。
//!
//! Zed 在 `crates/workspace/src/item.rs` 定义了 ~50 个方法的大 trait +
//! ItemSettings + PreviewTabsSettings + TabContentParams 等。
//!
//! AAgent 拆分成子模块：
//! - [handle] — ItemHandle（强引用）+ WeakItemHandle（弱引用）+ Item（强类型）
//! - [settings] — ItemSettings / PreviewTabsSettings / ClosePosition 等
//! - [tab] — TabContentParams / TabTooltipContent / ItemBufferKind

mod events;
mod follow;
mod handle;
mod handle_impl;
mod project_item;
mod serializable;
pub mod settings;
pub mod tab;
mod traits;
pub mod weak_handle;

pub use events::*;
pub use follow::*;
pub use handle::*;
pub use handle_impl::*;
pub use project_item::*;
pub use serializable::*;
pub use settings::*;
pub use tab::{ItemBufferKind, TabContentParams, TabTooltipContent};
pub use traits::*;
pub use weak_handle::*;

use std::time::Duration;

pub const LEADER_UPDATE_THROTTLE: Duration = Duration::from_millis(200);
