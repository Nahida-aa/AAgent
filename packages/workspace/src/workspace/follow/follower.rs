use super::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Follower {
    pub(crate) project_id: Option<u64>,
    pub(crate) peer_id: PeerId,
}

// 我是 follower 时做什么
impl Workspace {
    //
    pub(crate) fn leader_updated(
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
    //
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

    // 读的是 follower_states——那是 follower 侧的状态表。它回答的是“我这个 follower 的 pane 跟着谁”
    pub fn leader_for_pane(&self, pane: &Entity<Pane>) -> Option<CollaboratorId> {
        self.follower_states.iter().find_map(|(leader_id, state)| {
            if state.center_pane == *pane || state.dock_pane.as_ref() == Some(pane) {
                Some(*leader_id)
            } else {
                None
            }
        })
    }
}
