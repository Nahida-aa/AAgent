//! Dock 面板容器，对齐 zed `dock.rs`。

mod buttons;
pub mod buttons;
mod entity;
pub mod panel;
mod panel;
mod position;
mod size;

pub use buttons::PanelButtons;
pub use entity::Dock;
pub use panel::{Panel, PanelEvent, PanelHandle};
pub use position::DockPosition;
pub use size::PanelSizeState;

pub use proto::PanelId;
use std::sync::Arc;

use gpui::Pixels;
use gpui::px;

pub(crate) const RESIZE_HANDLE_SIZE: Pixels = px(6.);

pub(crate) const PANEL_SIZE_STATE_KEY: &str = "dock_panel_size";

use aa_gpui_kit_theme::ActiveTheme;
use gpui::{
    App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement,
    Render, Styled, Window, deferred, div, hsla, prelude::*, px,
};

use self::panel::{Panel, PanelHandle};

/// Resize handle 的大小（对齐 zed dock.rs `RESIZE_HANDLE_SIZE = px(6.)`）。
pub const RESIZE_HANDLE_SIZE: f32 = 6.0;
