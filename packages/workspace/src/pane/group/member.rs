use std::sync::Arc;

use gpui::{AnyWeakView, App, Entity, IntoElement, StyleRefinement, WeakEntity, Window, div};
use ui::prelude::*;

use crate::Pane;

use super::axis::PaneAxis;
use super::render::{PaneLeaderDecorator, PaneRenderResult};
use super::split_direction::SplitDirection;
use super::{HANDLE_HITBOX_SIZE, HORIZONTAL_MIN_SIZE, VERTICAL_MIN_SIZE};

/// PaneGroup 的一个 child。
#[derive(Debug, Clone)]
pub enum Member {
    /// 携带 members: Vec<Member>,
    Axis(PaneAxis),
    /// 叶子 Pane。
    Pane(Entity<Pane>),
}

impl Member {
    pub fn mark_positions(&mut self, in_center_group: bool, cx: &mut App) {
        match self {
            Member::Axis(pane_axis) => {
                for member in pane_axis.members.iter_mut() {
                    member.mark_positions(in_center_group, cx);
                }
            }
            Member::Pane(entity) => entity.update(cx, |pane, _| {
                pane.in_center_group = in_center_group;
            }),
        }
    }

    fn full_height_column_count(&self) -> usize {
        match self {
            Member::Pane(_) => 1,
            Member::Axis(axis) => axis.full_height_column_count(),
        }
    }
}
