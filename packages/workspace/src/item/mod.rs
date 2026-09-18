//! Item — Pane 里装的东西的统一接口。
//!
//! Zed 在 `crates/workspace/src/item.rs` 定义了 ~50 个方法的大 trait +
//! ItemSettings + PreviewTabsSettings + TabContentParams 等。
//!
//! AAgent 拆分成子模块：
//! - [handle] — ItemHandle（强引用）+ WeakItemHandle（弱引用）+ Item（强类型）
//! - [settings] — ItemSettings / PreviewTabsSettings / ClosePosition 等
//! - [tab] — TabContentParams / TabTooltipContent / ItemBufferKind

pub mod handle;
pub mod settings;
pub mod tab;

pub use handle::{Item, ItemHandle, WeakItemHandle};
pub use settings::{
    ActivateOnClose, ClosePosition, ItemSettings, PreviewTabsSettings, ShowCloseButton,
    ShowDiagnostics,
};
pub use tab::{ItemBufferKind, TabContentParams, TabTooltipContent};
