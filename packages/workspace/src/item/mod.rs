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
pub use ::settings::{
    ActivateOnClose, ClosePosition, RegisterSetting, Settings, SettingsLocation, ShowCloseButton,
    ShowDiagnostics,
};
pub use crate::item::settings::{ItemSettings,PreviewTabsSettings};
pub use tab::{ TabContentParams, TabTooltipContent};
pub use traits::*;
pub use weak_handle::*;

use std::time::Duration;

pub const LEADER_UPDATE_THROTTLE: Duration = Duration::from_millis(200);

/// Item buffer 类型 — 对齐 Zed `ItemBufferKind`。
/// Zed 用来判断 tab 是否显示 split marker、buffer 数量等。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemBufferKind {
    /// 多个 buffer（比如 Search Results multibuffer）。
    Multibuffer,
    /// 单实例（比如 Editor 一个文件一个实例）。
    Singleton,
    /// 不适用（Terminal 没有 buffer 概念）。
    None,
}

impl Default for ItemBufferKind {
    fn default() -> Self { Self::None }
}
