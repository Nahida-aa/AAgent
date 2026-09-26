//! Dock 面板容器，对齐 zed `dock.rs`。

pub mod buttons;
mod entity;
pub mod panel;
mod position;
mod size;

pub use buttons::PanelButtons;
pub use entity::Dock;
pub use panel::{Panel, PanelEvent, PanelHandle};
pub use position::DockPosition;
pub use size::PanelSizeState;

pub use proto::PanelId;

// 外部 crate 导入（子模块通过 use super::* 继承）
use std::sync::Arc;
use gpui::{
    App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement,
    Pixels, Render, Styled, Window, deferred, div, hsla, px, prelude::*,
};
use theme::ActiveTheme;

pub(crate) const RESIZE_HANDLE_SIZE: Pixels = px(6.);

pub(crate) const PANEL_SIZE_STATE_KEY: &str = "dock_panel_size";
