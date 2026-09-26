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

use anyhow::Context as _;
use client::proto;
use db::kvp::KeyValueStore;
use gpui::{
    Action, Anchor, AnyView, App, Axis, Context, Entity, EntityId, EventEmitter, FocusHandle,
    Focusable, IntoElement, KeyContext, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement,
    Pixels, Render, SharedString, StyleRefinement, Styled, Subscription, WeakEntity, Window,
    deferred, div, hsla, px, prelude::*,
};
use serde::{Deserialize, Serialize};
use settings::{Settings, SettingsStore};
use theme::ActiveTheme;
use ui::{ContextMenu, IconButton, Tooltip, prelude::*, right_click_menu};
use util::ResultExt as _;

// crate 根模块导入（子模块通过 use super::* 继承）
use crate::{
    DraggedDock, Event, FocusFollowsMouse, ModalLayer, Pane, Workspace,
    WorkspaceSettings, focus_follows_mouse::FocusFollowsMouse as _,
    persistence::model::DockData,
};

pub(crate) const RESIZE_HANDLE_SIZE: Pixels = px(6.);

pub(crate) const PANEL_SIZE_STATE_KEY: &str = "dock_panel_size";
