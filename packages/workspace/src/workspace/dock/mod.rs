use super::*;
use gpui::{
    App, Axis, Context, Div, DragMoveEvent, Entity, IntoElement, ParentElement, Pixels, Role,
    Stateful, Styled, Window, div, px,
};

use std::sync::Arc;

use super::Workspace;
pub(crate) use crate::dock::{
    Dock, DockPosition, Panel, PanelButtons, PanelHandle, PanelSizeState, RESIZE_HANDLE_SIZE,
};
use crate::pane::group::PaneRenderContext;
use crate::workspace::core::actions::ToggleAllDocks;

// Dock 是左/右/底三个区域之一，管理开合、当前显示哪个 panel、各 panel 的尺寸状态
// Panel 是具体面板（终端、项目、Outline…），提供默认尺寸和是否可伸缩
// 关系是 1 Dock : N Panel，同一时刻只有 1 个 active panel 可见
// 尺寸状态存在 Dock 里，不在 Panel 里。因为尺寸是容器决定的——受窗口大小、其他 dock、center 列数影响，Panel 自己不知道这些

// left_dock
// right_dock
// bottom_dock
// all_docks
// dock_at_position
// focused_dock_position
// agent_panel_position
mod read;
mod ops;
mod state;
pub(crate) mod sizing;
pub mod render;
