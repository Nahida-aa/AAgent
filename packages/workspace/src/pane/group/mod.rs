//! PaneGroup — 递归 split 树，对齐 zed `crates/workspace/src/pane_group.rs`。
//!
//! Zed PaneGroup 是普通 struct（非 GPUI entity），AAgent 保持一致。
//! - `Member::Pane(Entity<Pane>)` — 叶子
//! - `Member::PaneGroup(PaneGroup)` — 内部节点（可以继续 split）
//! - `axis: Axis` — 叶子们怎么排（Horizontal=上下, Vertical=左右）

use gpui::{AnyElement, App, Axis, Entity, IntoElement, div, prelude::*};

use crate::pane::Pane;

mod axis;
mod element;
mod group;
mod member;
mod render;
mod split_direction;

pub use axis::PaneAxis;
pub use group::PaneGroup;
pub use member::Member;
pub use render::{
    ActivePaneDecorator, LeaderDecoration, PaneLeaderDecorator, PaneRenderContext, PaneRenderResult,
};
pub use split_direction::SplitDirection;

pub const HANDLE_HITBOX_SIZE: f32 = 4.0;
pub(crate) const HORIZONTAL_MIN_SIZE: f32 = 80.;
pub(crate) const VERTICAL_MIN_SIZE: f32 = 100.;

pub(crate) use element::pane_axis;
