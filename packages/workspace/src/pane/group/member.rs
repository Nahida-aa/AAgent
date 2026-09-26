use std::sync::Arc;

// use gpui::{AnyWeakView, App, Entity, IntoElement, StyleRefinement, WeakEntity, Window, div};
use gpui::{
    Along, AnyView, AnyWeakView, Axis, Bounds, Entity, Hsla, IntoElement, MouseButton, Pixels,
    Point, StyleRefinement, WeakEntity, Window, point, size,
};
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
    pub(crate) fn new_axis(
        old_pane: Entity<Pane>,
        new_pane: Entity<Pane>,
        direction: SplitDirection,
    ) -> Self {
        use Axis::*;
        use SplitDirection::*;

        let axis = match direction {
            Up | Down => Vertical,
            Left | Right => Horizontal,
        };

        let members = match direction {
            Up | Left => vec![Member::Pane(new_pane), Member::Pane(old_pane)],
            Down | Right => vec![Member::Pane(old_pane), Member::Pane(new_pane)],
        };

        Member::Axis(PaneAxis::new(axis, members))
    }

    pub(crate) fn first_pane(&self) -> Entity<Pane> {
        match self {
            Member::Axis(axis) => axis.members[0].first_pane(),
            Member::Pane(pane) => pane.clone(),
        }
    }

    pub(crate) fn last_pane(&self) -> Entity<Pane> {
        match self {
            Member::Axis(axis) => axis.members.last().unwrap().last_pane(),
            Member::Pane(pane) => pane.clone(),
        }
    }

    pub fn render(
        &self,
        basis: usize,
        zoomed: Option<&AnyWeakView>,
        maximized: Option<&WeakEntity<Pane>>,
        render_cx: &dyn PaneLeaderDecorator,
        window: &mut Window,
        cx: &mut App,
    ) -> PaneRenderResult {
        match self {
            Member::Pane(pane) => {
                if zoomed == Some(&pane.downgrade().into()) {
                    return PaneRenderResult {
                        element: div().into_any(),
                        contains_active_pane: false,
                        #[cfg(any(test, feature = "test-support"))]
                        decorated_pane_ix: None,
                    };
                }

                let is_maximized = if let Some(maximized) = maximized {
                    if maximized.upgrade().as_ref() != Some(pane) {
                        return PaneRenderResult {
                            element: div().into_any(),
                            contains_active_pane: false,
                            #[cfg(any(test, feature = "test-support"))]
                            decorated_pane_ix: None,
                        };
                    }
                    true
                } else {
                    false
                };

                let decoration = render_cx.decorate(pane, cx);
                let is_active = pane == render_cx.active_pane();

                let pane = div()
                    .relative()
                    .size_full()
                    .when(is_maximized, |this| {
                        this.bg(cx.theme().colors().background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .shadow_lg()
                            .overflow_hidden()
                    })
                    .child(
                        AnyView::from(pane.clone())
                            .cached(StyleRefinement::default().v_flex().size_full()),
                    )
                    .when_some(decoration.border, |this, color| {
                        this.child(
                            div()
                                .absolute()
                                .size_full()
                                .left_0()
                                .top_0()
                                .border_2()
                                .border_color(color),
                        )
                    })
                    .children(decoration.status_box);

                PaneRenderResult {
                    element: div()
                        .relative()
                        .flex_1()
                        .size_full()
                        .when(is_maximized, |this| this.p_2())
                        .child(pane)
                        .into_any(),
                    contains_active_pane: is_active,
                    #[cfg(any(test, feature = "test-support"))]
                    decorated_pane_ix: None,
                }
            }
            Member::Axis(axis) => axis.render(basis + 1, zoomed, maximized, render_cx, window, cx),
        }
    }

    pub fn contains_pane(&self, needle: &Entity<Pane>) -> bool {
        match self {
            Member::Pane(pane) => pane == needle,
            Member::Axis(axis) => axis
                .members
                .iter()
                .any(|member| member.contains_pane(needle)),
        }
    }

    pub(crate) fn collect_panes<'a>(&'a self, panes: &mut Vec<&'a Entity<Pane>>) {
        match self {
            Member::Axis(axis) => {
                for member in &axis.members {
                    member.collect_panes(panes);
                }
            }
            Member::Pane(pane) => panes.push(pane),
        }
    }

    pub(crate) fn invert_pane_axies(&mut self) {
        match self {
            Self::Axis(axis) => {
                axis.axis = axis.axis.invert();
                for member in axis.members.iter_mut() {
                    member.invert_pane_axies();
                }
            }
            Self::Pane(_) => {}
        }
    }
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

    pub(crate) fn full_height_column_count(&self) -> usize {
        match self {
            Member::Pane(_) => 1,
            Member::Axis(axis) => axis.full_height_column_count(),
        }
    }
}
