use std::sync::Arc;

use collections::HashMap;
use gpui::{
    AnyElement, AnyWeakView, App, Entity, Hsla, IntoElement, MouseButton, WeakEntity, Window,
};
use ui::prelude::*;
use util::ResultExt;

use crate::notifications::DetachAndPromptErr;
use crate::{
    AnyActiveCall, AppState, CollaboratorId, FollowerState, Pane, ParticipantLocation, Workspace,
};

use super::SplitDirection;

pub struct PaneRenderResult {
    pub element: gpui::AnyElement,
    pub contains_active_pane: bool,
    #[cfg(any(test, feature = "test-support"))]
    pub decorated_pane_ix: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct PaneRenderContext<'a> {
    pub project: &'a Entity<Project>,
    pub follower_states: &'a HashMap<CollaboratorId, FollowerState>,
    pub active_call: Option<&'a dyn AnyActiveCall>,
    pub active_pane: &'a Entity<Pane>,
    pub app_state: &'a Arc<AppState>,
    pub workspace: &'a WeakEntity<Workspace>,
}

#[derive(Default)]
pub struct LeaderDecoration {
    pub(super) border: Option<Hsla>,
    pub(super) status_box: Option<AnyElement>,
}

pub trait PaneLeaderDecorator {
    fn decorate(&self, pane: &Entity<Pane>, cx: &App) -> LeaderDecoration;
    fn active_pane(&self) -> &Entity<Pane>;
    fn workspace(&self) -> &WeakEntity<Workspace>;
}

pub struct ActivePaneDecorator<'a> {
    active_pane: &'a Entity<Pane>,
    workspace: &'a WeakEntity<Workspace>,
}

impl<'a> ActivePaneDecorator<'a> {
    pub fn new(active_pane: &'a Entity<Pane>, workspace: &'a WeakEntity<Workspace>) -> Self {
        Self {
            active_pane,
            workspace,
        }
    }
}

impl PaneLeaderDecorator for ActivePaneDecorator<'_> {
    fn decorate(&self, _: &Entity<Pane>, _: &App) -> LeaderDecoration {
        LeaderDecoration::default()
    }
    fn active_pane(&self) -> &Entity<Pane> { self.active_pane }

    fn workspace(&self) -> &WeakEntity<Workspace> { self.workspace }
}

impl PaneLeaderDecorator for PaneRenderContext<'_> {
    fn decorate(&self, pane: &Entity<Pane>, cx: &App) -> LeaderDecoration {
        let follower_state = self.follower_states.iter().find_map(|(leader_id, state)| {
            if state.center_pane == *pane {
                Some((*leader_id, state))
            } else {
                None
            }
        });
        let Some((leader_id, follower_state)) = follower_state else {
            return LeaderDecoration::default();
        };

        let mut leader_color;
        let status_box;
        match leader_id {
            CollaboratorId::PeerId(peer_id) => {
                let Some(leader) = self
                    .active_call
                    .as_ref()
                    .and_then(|call| call.remote_participant_for_peer_id(peer_id, cx))
                else {
                    return LeaderDecoration::default();
                };

                let is_in_unshared_view = follower_state.active_view_id.is_some_and(|view_id| {
                    !follower_state
                        .items_by_leader_view_id
                        .contains_key(&view_id)
                });

                let mut leader_join_data = None;
                let leader_status_box = match leader.location {
                    ParticipantLocation::SharedProject {
                        project_id: leader_project_id,
                    } => {
                        if Some(leader_project_id) == self.project.read(cx).remote_id() {
                            is_in_unshared_view.then(|| {
                                Label::new(format!(
                                    "{} is in an unshared pane",
                                    leader.user.username
                                ))
                            })
                        } else {
                            leader_join_data = Some((leader_project_id, leader.user.legacy_id));
                            Some(Label::new(format!(
                                "Follow {} to their active project",
                                leader.user.username,
                            )))
                        }
                    }
                    ParticipantLocation::UnsharedProject => Some(Label::new(format!(
                        "{} is viewing an unshared Zed project",
                        leader.user.username
                    ))),
                    ParticipantLocation::External => Some(Label::new(format!(
                        "{} is viewing a window outside of Zed",
                        leader.user.username
                    ))),
                };
                status_box = leader_status_box.map(|status| {
                    div()
                        .absolute()
                        .w_96()
                        .bottom_3()
                        .right_3()
                        .elevation_2(cx)
                        .p_1()
                        .child(status)
                        .when_some(
                            leader_join_data,
                            |this, (leader_project_id, leader_user_id)| {
                                let app_state = self.app_state.clone();
                                this.cursor_pointer().on_mouse_down(
                                    MouseButton::Left,
                                    move |_, window, cx| {
                                        crate::join_in_room_project(
                                            leader_project_id,
                                            leader_user_id,
                                            app_state.clone(),
                                            cx,
                                        )
                                        .detach_and_prompt_err(
                                            "Failed to join project",
                                            window,
                                            cx,
                                            |error, _, _| Some(format!("{error:#}")),
                                        );
                                    },
                                )
                            },
                        )
                        .into_any_element()
                });
                leader_color = cx
                    .theme()
                    .players()
                    .color_for_participant(leader.participant_index.0)
                    .cursor;
            }
            CollaboratorId::Agent => {
                status_box = None;
                leader_color = cx.theme().players().agent().cursor;
            }
        }

        let is_in_panel = follower_state.dock_pane.is_some();
        if is_in_panel {
            leader_color.fade_out(0.75);
        } else {
            leader_color.fade_out(0.3);
        }

        LeaderDecoration {
            status_box,
            border: Some(leader_color),
        }
    }

    fn active_pane(&self) -> &Entity<Pane> { self.active_pane }

    fn workspace(&self) -> &WeakEntity<Workspace> { self.workspace }
}
