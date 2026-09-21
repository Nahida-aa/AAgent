use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

pub struct FollowerState {
    center_pane: Entity<Pane>,
    dock_pane: Option<Entity<Pane>>,
    active_view_id: Option<ViewId>,
    items_by_leader_view_id: HashMap<ViewId, FollowerView>,
}

struct FollowerView {
    view: Box<dyn FollowableItemHandle>,
    location: Option<proto::PanelId>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Follower {
    project_id: Option<u64>,
    peer_id: PeerId,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum CollaboratorId {
    PeerId(PeerId),
    Agent,
}

impl From<PeerId> for CollaboratorId {
    fn from(peer_id: PeerId) -> Self { CollaboratorId::PeerId(peer_id) }
}

impl From<&PeerId> for CollaboratorId {
    fn from(peer_id: &PeerId) -> Self { CollaboratorId::PeerId(*peer_id) }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewId {
    pub creator: CollaboratorId,
    pub id: u64,
}

impl Workspace {
    pub fn start_following(
        &mut self,
        leader_id: impl Into<CollaboratorId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let leader_id = leader_id.into();
        let pane = self.active_pane().clone();

        self.last_leaders_by_pane
            .insert(pane.downgrade(), leader_id);
        self.unfollow(leader_id, window, cx);
        self.unfollow_in_pane(&pane, window, cx);
        self.auto_watch = AutoWatch::Off;
        self.follower_states.insert(
            leader_id,
            FollowerState {
                center_pane: pane.clone(),
                dock_pane: None,
                active_view_id: None,
                items_by_leader_view_id: Default::default(),
            },
        );
        cx.notify();

        match leader_id {
            CollaboratorId::PeerId(leader_peer_id) => {
                let room_id = self.active_call()?.room_id(cx)?;
                let project_id = self.project.read(cx).remote_id();
                let request = self.app_state.client.request(proto::Follow {
                    room_id,
                    project_id,
                    leader_id: Some(leader_peer_id),
                });

                Some(cx.spawn_in(window, async move |this, cx| {
                    let response = request.await?;
                    this.update(cx, |this, _| {
                        let state = this
                            .follower_states
                            .get_mut(&leader_id)
                            .context("following interrupted")?;
                        state.active_view_id = response
                            .active_view
                            .as_ref()
                            .and_then(|view| ViewId::from_proto(view.id?).ok());
                        anyhow::Ok(())
                    })??;
                    if let Some(view) = response.active_view {
                        Self::add_view_from_leader(this.clone(), leader_peer_id, &view, cx).await?;
                    }
                    this.update_in(cx, |this, window, cx| {
                        this.leader_updated(leader_id, window, cx)
                    })?;
                    Ok(())
                }))
            }
            CollaboratorId::Agent => {
                self.leader_updated(leader_id, window, cx)?;
                Some(Task::ready(Ok(())))
            }
        }
    }

    pub fn follow_next_collaborator(
        &mut self,
        _: &FollowNextCollaborator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let collaborators = self.project.read(cx).collaborators();
        let next_leader_id = if let Some(leader_id) = self.leader_for_pane(&self.active_pane) {
            let mut collaborators = collaborators.keys().copied();
            for peer_id in collaborators.by_ref() {
                if CollaboratorId::PeerId(peer_id) == leader_id {
                    break;
                }
            }
            collaborators.next().map(CollaboratorId::PeerId)
        } else if let Some(last_leader_id) =
            self.last_leaders_by_pane.get(&self.active_pane.downgrade())
        {
            match last_leader_id {
                CollaboratorId::PeerId(peer_id) => {
                    if collaborators.contains_key(peer_id) {
                        Some(*last_leader_id)
                    } else {
                        None
                    }
                }
                CollaboratorId::Agent => Some(CollaboratorId::Agent),
            }
        } else {
            None
        };

        let pane = self.active_pane.clone();
        let Some(leader_id) = next_leader_id.or_else(|| {
            Some(CollaboratorId::PeerId(
                collaborators.keys().copied().next()?,
            ))
        }) else {
            return;
        };
        if self.unfollow_in_pane(&pane, window, cx) == Some(leader_id) {
            return;
        }
        if let Some(task) = self.start_following(leader_id, window, cx) {
            task.detach_and_log_err(cx)
        }
    }

    pub fn follow(
        &mut self,
        leader_id: impl Into<CollaboratorId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let leader_id = leader_id.into();

        if let CollaboratorId::PeerId(peer_id) = leader_id {
            let Some(active_call) = GlobalAnyActiveCall::try_global(cx) else {
                return;
            };
            let Some(remote_participant) =
                active_call.0.remote_participant_for_peer_id(peer_id, cx)
            else {
                return;
            };

            let project = self.project.read(cx);

            let other_project_id = match remote_participant.location {
                ParticipantLocation::External => None,
                ParticipantLocation::UnsharedProject => None,
                ParticipantLocation::SharedProject { project_id } => {
                    if Some(project_id) == project.remote_id() {
                        None
                    } else {
                        Some(project_id)
                    }
                }
            };

            // if they are active in another project, follow there.
            if let Some(project_id) = other_project_id {
                let app_state = self.app_state.clone();
                crate::join_in_room_project(
                    project_id,
                    remote_participant.user.legacy_id,
                    app_state,
                    cx,
                )
                .detach_and_prompt_err(
                    "Failed to join project",
                    window,
                    cx,
                    |error, _, _| Some(format!("{error:#}")),
                );
            }
        }

        // if you're already following, find the right pane and focus it.
        if let Some(follower_state) = self.follower_states.get(&leader_id) {
            window.focus(&follower_state.pane().focus_handle(cx), cx);

            return;
        }

        // Otherwise, follow.
        if let Some(task) = self.start_following(leader_id, window, cx) {
            task.detach_and_log_err(cx)
        }
    }

    pub fn unfollow(
        &mut self,
        leader_id: impl Into<CollaboratorId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<()> {
        cx.notify();

        let leader_id = leader_id.into();
        let state = self.follower_states.remove(&leader_id)?;
        for (_, item) in state.items_by_leader_view_id {
            item.view.set_leader_id(None, window, cx);
        }

        if let CollaboratorId::PeerId(leader_peer_id) = leader_id {
            let project_id = self.project.read(cx).remote_id();
            let room_id = self.active_call()?.room_id(cx)?;
            self.app_state
                .client
                .send(proto::Unfollow {
                    room_id,
                    project_id,
                    leader_id: Some(leader_peer_id),
                })
                .log_err();
        }

        Some(())
    }
    pub fn unfollow_in_pane(
        &mut self,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Option<CollaboratorId> {
        let leader_id = self.leader_for_pane(pane)?;
        self.unfollow(leader_id, window, cx);
        Some(leader_id)
    }
    pub fn is_being_followed(&self, id: impl Into<CollaboratorId>) -> bool {
        self.follower_states.contains_key(&id.into())
    }

    pub fn leader_for_pane(&self, pane: &Entity<Pane>) -> Option<CollaboratorId> {
        self.follower_states.iter().find_map(|(leader_id, state)| {
            if state.center_pane == *pane || state.dock_pane.as_ref() == Some(pane) {
                Some(*leader_id)
            } else {
                None
            }
        })
    }

    fn leader_updated(
        &mut self,
        leader_id: impl Into<CollaboratorId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Box<dyn ItemHandle>> {
        cx.notify();

        let leader_id = leader_id.into();
        let (panel_id, item) = match leader_id {
            CollaboratorId::PeerId(peer_id) => self.active_item_for_peer(peer_id, window, cx)?,
            CollaboratorId::Agent => (None, self.active_item_for_agent()?),
        };

        let state = self.follower_states.get(&leader_id)?;
        let mut transfer_focus = state.center_pane.read(cx).has_focus(window, cx);
        let pane;
        if let Some(panel_id) = panel_id {
            pane = self
                .activate_panel_for_proto_id(panel_id, window, cx)?
                .pane(cx)?;
            let state = self.follower_states.get_mut(&leader_id)?;
            state.dock_pane = Some(pane.clone());
        } else {
            pane = state.center_pane.clone();
            let state = self.follower_states.get_mut(&leader_id)?;
            if let Some(dock_pane) = state.dock_pane.take() {
                transfer_focus |= dock_pane.focus_handle(cx).contains_focused(window, cx);
            }
        }

        pane.update(cx, |pane, cx| {
            let focus_active_item = pane.has_focus(window, cx) || transfer_focus;
            if let Some(index) = pane.index_for_item(item.as_ref()) {
                pane.activate_item(index, false, false, window, cx);
            } else {
                pane.add_item(item.boxed_clone(), false, false, None, window, cx)
            }

            if focus_active_item {
                pane.focus_active_item(window, cx)
            }
        });

        Some(item)
    }

    fn collaborator_left(&mut self, peer_id: PeerId, window: &mut Window, cx: &mut Context<Self>) {
        self.follower_states.retain(|leader_id, state| {
            if *leader_id == CollaboratorId::PeerId(peer_id) {
                for item in state.items_by_leader_view_id.values() {
                    item.view.set_leader_id(None, window, cx);
                }
                false
            } else {
                true
            }
        });
        cx.notify();
    }

    fn active_item_for_peer(
        &self,
        peer_id: PeerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(Option<PanelId>, Box<dyn ItemHandle>)> {
        let call = self.active_call()?;
        let participant = call.remote_participant_for_peer_id(peer_id, cx)?;
        let leader_in_this_app;
        let leader_in_this_project;
        match participant.location {
            ParticipantLocation::SharedProject { project_id } => {
                leader_in_this_app = true;
                leader_in_this_project = Some(project_id) == self.project.read(cx).remote_id();
            }
            ParticipantLocation::UnsharedProject => {
                leader_in_this_app = true;
                leader_in_this_project = false;
            }
            ParticipantLocation::External => {
                leader_in_this_app = false;
                leader_in_this_project = false;
            }
        };
        let state = self.follower_states.get(&peer_id.into())?;
        let mut item_to_activate = None;
        if let (Some(active_view_id), true) = (state.active_view_id, leader_in_this_app) {
            if let Some(item) = state.items_by_leader_view_id.get(&active_view_id)
                && (leader_in_this_project || !item.view.is_project_item(window, cx))
            {
                item_to_activate = Some((item.location, item.view.boxed_clone()));
            }
        } else if let Some(shared_screen) =
            self.shared_screen_for_peer(peer_id, &state.center_pane, window, cx)
        {
            item_to_activate = Some((None, Box::new(shared_screen)));
        }
        item_to_activate
    }

    fn active_item_for_agent(&self) -> Option<Box<dyn ItemHandle>> {
        let state = self.follower_states.get(&CollaboratorId::Agent)?;
        let active_view_id = state.active_view_id?;
        Some(
            state
                .items_by_leader_view_id
                .get(&active_view_id)?
                .view
                .boxed_clone(),
        )
    }

    fn active_view_for_follower(
        &self,
        follower_project_id: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<proto::View> {
        let (item, panel_id) = self.active_item_for_followers(window, cx);
        let item = item?;
        let leader_id = self
            .pane_for(&*item)
            .and_then(|pane| self.leader_for_pane(&pane));
        let leader_peer_id = match leader_id {
            Some(CollaboratorId::PeerId(peer_id)) => Some(peer_id),
            Some(CollaboratorId::Agent) | None => None,
        };

        let item_handle = item.to_followable_item_handle(cx)?;
        let id = item_handle.remote_id(&self.app_state.client, window, cx)?;
        let variant = item_handle.to_state_proto(window, cx)?;

        if item_handle.is_project_item(window, cx)
            && (follower_project_id.is_none()
                || follower_project_id != self.project.read(cx).remote_id())
        {
            return None;
        }

        Some(proto::View {
            id: id.to_proto(),
            leader_id: leader_peer_id,
            variant: Some(variant),
            panel_id: panel_id.map(|id| id as i32),
        })
    }

    fn active_item_for_followers(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> (Option<Box<dyn ItemHandle>>, Option<proto::PanelId>) {
        let mut active_item = None;
        let mut panel_id = None;
        for dock in self.all_docks() {
            if dock.focus_handle(cx).contains_focused(window, cx)
                && let Some(panel) = dock.read(cx).active_panel()
                && let Some(pane) = panel.pane(cx)
                && let Some(item) = pane.read(cx).active_item()
            {
                active_item = Some(item);
                panel_id = panel.remote_id();
                break;
            }
        }

        if active_item.is_none() {
            active_item = self.active_pane().read(cx).active_item();
        }
        (active_item, panel_id)
    }

    pub async fn handle_follow(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::Follow>,
        mut cx: AsyncApp,
    ) -> Result<proto::FollowResponse> {
        this.update(&mut cx, |this, cx| {
            let follower = Follower {
                project_id: envelope.payload.project_id,
                peer_id: envelope.original_sender_id()?,
            };

            let mut response = proto::FollowResponse::default();

            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let handler_response =
                                workspace.handle_follow(follower.project_id, window, cx);
                            if let Some(active_view) = handler_response.active_view
                                && workspace.project.read(cx).remote_id() == follower.project_id
                            {
                                response.active_view = Some(active_view)
                            }
                        });
                    })
                    .is_ok()
            });

            Ok(response)
        })
    }

    async fn handle_update_followers(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateFollowers>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let leader_id = envelope.original_sender_id()?;
        let update = envelope.payload;

        this.update(&mut cx, |this, cx| {
            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let project_id = workspace.project.read(cx).remote_id();
                            if update.project_id != project_id && update.project_id.is_some() {
                                return;
                            }
                            workspace.handle_update_followers(
                                leader_id,
                                update.clone(),
                                window,
                                cx,
                            );
                        });
                    })
                    .is_ok()
            });
            Ok(())
        })
    }
    pub(super) async fn process_leader_update(
        this: &WeakEntity<Self>,
        leader_id: PeerId,
        update: proto::UpdateFollowers,
        cx: &mut AsyncWindowContext,
    ) -> Result<()> {
        match update.variant.context("invalid update")? {
            proto::update_followers::Variant::CreateView(view) => {
                let view_id = ViewId::from_proto(view.id.context("invalid view id")?)?;
                let should_add_view = this.update(cx, |this, _| {
                    if let Some(state) = this.follower_states.get_mut(&leader_id.into()) {
                        anyhow::Ok(!state.items_by_leader_view_id.contains_key(&view_id))
                    } else {
                        anyhow::Ok(false)
                    }
                })??;

                if should_add_view {
                    Self::add_view_from_leader(this.clone(), leader_id, &view, cx).await?
                }
            }
            proto::update_followers::Variant::UpdateActiveView(update_active_view) => {
                let should_add_view = this.update(cx, |this, _| {
                    if let Some(state) = this.follower_states.get_mut(&leader_id.into()) {
                        state.active_view_id = update_active_view
                            .view
                            .as_ref()
                            .and_then(|view| ViewId::from_proto(view.id?).ok());

                        if state.active_view_id.is_some_and(|view_id| {
                            !state.items_by_leader_view_id.contains_key(&view_id)
                        }) {
                            anyhow::Ok(true)
                        } else {
                            anyhow::Ok(false)
                        }
                    } else {
                        anyhow::Ok(false)
                    }
                })??;

                if should_add_view && let Some(view) = update_active_view.view {
                    Self::add_view_from_leader(this.clone(), leader_id, &view, cx).await?
                }
            }
            proto::update_followers::Variant::UpdateView(update_view) => {
                let variant = update_view.variant.context("missing update view variant")?;
                let id = update_view.id.context("missing update view id")?;
                let mut tasks = Vec::new();
                this.update_in(cx, |this, window, cx| {
                    let project = this.project.clone();
                    if let Some(state) = this.follower_states.get(&leader_id.into()) {
                        let view_id = ViewId::from_proto(id)?;
                        if let Some(item) = state.items_by_leader_view_id.get(&view_id) {
                            tasks.push(item.view.apply_update_proto(
                                &project,
                                variant.clone(),
                                window,
                                cx,
                            ));
                        }
                    }
                    anyhow::Ok(())
                })??;
                try_join_all(tasks).await.log_err();
            }
        }
        this.update_in(cx, |this, window, cx| {
            this.leader_updated(leader_id, window, cx)
        })?;
        Ok(())
    }

    async fn add_view_from_leader(
        this: WeakEntity<Self>,
        leader_id: PeerId,
        view: &proto::View,
        cx: &mut AsyncWindowContext,
    ) -> Result<()> {
        let this = this.upgrade().context("workspace dropped")?;

        let Some(id) = view.id else {
            anyhow::bail!("no id for view");
        };
        let id = ViewId::from_proto(id)?;
        let panel_id = view
            .panel_id
            .and_then(|value| proto::PanelId::try_from(value).ok());

        let pane = this.update(cx, |this, _cx| {
            let state = this
                .follower_states
                .get(&leader_id.into())
                .context("stopped following")?;
            anyhow::Ok(state.pane().clone())
        })?;
        let existing_item = pane.update_in(cx, |pane, window, cx| {
            let client = this.read(cx).client().clone();
            pane.items().find_map(|item| {
                let item = item.to_followable_item_handle(cx)?;
                if item.remote_id(&client, window, cx) == Some(id) {
                    Some(item)
                } else {
                    None
                }
            })
        })?;
        let item = if let Some(existing_item) = existing_item {
            existing_item
        } else {
            let variant = view.variant.clone();
            anyhow::ensure!(variant.is_some(), "missing view variant");

            let task = cx.update(|window, cx| {
                FollowableViewRegistry::from_state_proto(this.clone(), id, variant, window, cx)
            })?;

            let Some(task) = task else {
                anyhow::bail!(
                    "failed to construct view from leader (maybe from a different version of zed?)"
                );
            };

            let mut new_item = task.await?;
            pane.update_in(cx, |pane, window, cx| {
                let mut item_to_remove = None;
                for (ix, item) in pane.items().enumerate() {
                    if let Some(item) = item.to_followable_item_handle(cx) {
                        match new_item.dedup(item.as_ref(), window, cx) {
                            Some(item::Dedup::KeepExisting) => {
                                new_item =
                                    item.boxed_clone().to_followable_item_handle(cx).unwrap();
                                break;
                            }
                            Some(item::Dedup::ReplaceExisting) => {
                                item_to_remove = Some((ix, item.item_id()));
                                break;
                            }
                            None => {}
                        }
                    }
                }

                if let Some((ix, id)) = item_to_remove {
                    pane.remove_item(id, false, false, window, cx);
                    pane.add_item(new_item.boxed_clone(), false, false, Some(ix), window, cx);
                }
            })?;

            new_item
        };

        this.update_in(cx, |this, window, cx| {
            let state = this.follower_states.get_mut(&leader_id.into())?;
            item.set_leader_id(Some(leader_id.into()), window, cx);
            state.items_by_leader_view_id.insert(
                id,
                FollowerView {
                    view: item,
                    location: panel_id,
                },
            );

            Some(())
        })
        .context("no follower state")?;

        Ok(())
    }

    fn handle_agent_location_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(follower_state) = self.follower_states.get_mut(&CollaboratorId::Agent) else {
            return;
        };

        if let Some(agent_location) = self.project.read(cx).agent_location() {
            let buffer_entity_id = agent_location.buffer.entity_id();
            let view_id = ViewId {
                creator: CollaboratorId::Agent,
                id: buffer_entity_id.as_u64(),
            };
            follower_state.active_view_id = Some(view_id);

            let item = match follower_state.items_by_leader_view_id.entry(view_id) {
                hash_map::Entry::Occupied(entry) => Some(entry.into_mut()),
                hash_map::Entry::Vacant(entry) => {
                    let existing_view =
                        follower_state
                            .center_pane
                            .read(cx)
                            .items()
                            .find_map(|item| {
                                let item = item.to_followable_item_handle(cx)?;
                                if item.buffer_kind(cx) == ItemBufferKind::Singleton
                                    && item.project_item_model_ids(cx).as_slice()
                                        == [buffer_entity_id]
                                {
                                    Some(item)
                                } else {
                                    None
                                }
                            });
                    let view = existing_view.or_else(|| {
                        agent_location.buffer.upgrade().and_then(|buffer| {
                            cx.update_default_global(|registry: &mut ProjectItemRegistry, cx| {
                                registry.build_item(buffer, self.project.clone(), None, window, cx)
                            })?
                            .to_followable_item_handle(cx)
                        })
                    });

                    view.map(|view| {
                        entry.insert(FollowerView {
                            view,
                            location: None,
                        })
                    })
                }
            };

            if let Some(item) = item {
                item.view
                    .set_leader_id(Some(CollaboratorId::Agent), window, cx);
                item.view
                    .update_agent_location(agent_location.position, window, cx);
            }
        } else {
            follower_state.active_view_id = None;
        }

        self.leader_updated(CollaboratorId::Agent, window, cx);
    }

    pub fn update_active_view_for_followers(&mut self, window: &mut Window, cx: &mut App) {
        let mut is_project_item = true;
        let mut update = proto::UpdateActiveView::default();
        if window.is_window_active() {
            let (active_item, panel_id) = self.active_item_for_followers(window, cx);

            if let Some(item) = active_item
                && item.item_focus_handle(cx).contains_focused(window, cx)
            {
                let leader_id = self
                    .pane_for(&*item)
                    .and_then(|pane| self.leader_for_pane(&pane));
                let leader_peer_id = match leader_id {
                    Some(CollaboratorId::PeerId(peer_id)) => Some(peer_id),
                    Some(CollaboratorId::Agent) | None => None,
                };

                if let Some(item) = item.to_followable_item_handle(cx) {
                    let id = item
                        .remote_id(&self.app_state.client, window, cx)
                        .map(|id| id.to_proto());

                    if let Some(id) = id
                        && let Some(variant) = item.to_state_proto(window, cx)
                    {
                        let view = Some(proto::View {
                            id,
                            leader_id: leader_peer_id,
                            variant: Some(variant),
                            panel_id: panel_id.map(|id| id as i32),
                        });

                        is_project_item = item.is_project_item(window, cx);
                        update = proto::UpdateActiveView { view };
                    };
                }
            }
        }

        let active_view_id = update.view.as_ref().and_then(|view| view.id.as_ref());
        if active_view_id != self.last_active_view_id.as_ref() {
            self.last_active_view_id = active_view_id.cloned();
            self.update_followers(
                is_project_item,
                proto::update_followers::Variant::UpdateActiveView(update),
                window,
                cx,
            );
        }
    }

    fn update_followers(
        &self,
        project_only: bool,
        update: proto::update_followers::Variant,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<()> {
        // If this update only applies to for followers in the current project,
        // then skip it unless this project is shared. If it applies to all
        // followers, regardless of project, then set `project_id` to none,
        // indicating that it goes to all followers.
        let project_id = if project_only {
            Some(self.project.read(cx).remote_id()?)
        } else {
            None
        };
        self.app_state().workspace_store.update(cx, |store, cx| {
            store.update_followers(project_id, update, cx)
        })
    }

    fn shared_screen_for_peer(
        &self,
        peer_id: PeerId,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<SharedScreen>> {
        self.active_call()?
            .create_shared_screen(peer_id, pane, window, cx)
    }

    pub(super) fn on_active_call_event(
        &mut self,
        event: &ActiveCallEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ActiveCallEvent::ParticipantLocationChanged { participant_id } => {
                self.leader_updated(participant_id, window, cx);
            }
            ActiveCallEvent::RemoteVideoTracksChanged { participant_id } => {
                self.leader_updated(participant_id, window, cx);
                self.handle_auto_watch_video_tracks_changed(*participant_id, window, cx);
            }
            ActiveCallEvent::LocalScreenShareStarted => {
                if let AutoWatch::Active { .. } = self.auto_watch {
                    self.auto_watch = AutoWatch::Paused;
                    cx.notify();
                }
            }
            ActiveCallEvent::LocalScreenShareStopped => {
                self.handle_auto_watch_local_share_stopped(window, cx);
            }
            ActiveCallEvent::RoomLeft => {
                if self.auto_watch.enabled() {
                    self.auto_watch = AutoWatch::Off;
                    cx.notify();
                }
            }
        }
    }

    pub fn active_call(&self) -> Option<&dyn AnyActiveCall> {
        self.active_call.as_ref().map(|(call, _)| &*call.0)
    }

    pub fn active_global_call(&self) -> Option<GlobalAnyActiveCall> {
        self.active_call.as_ref().map(|(call, _)| call.clone())
    }
}
