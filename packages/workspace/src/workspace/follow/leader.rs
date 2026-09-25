impl Workspace {
    //
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
    //
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
    //
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
}

fn leader_border_for_pane(
    follower_states: &HashMap<CollaboratorId, FollowerState>,
    pane: &Entity<Pane>,
    _: &Window,
    cx: &App,
) -> Option<Div> {
    let (leader_id, _follower_state) = follower_states.iter().find_map(|(leader_id, state)| {
        if state.pane() == pane {
            Some((*leader_id, state))
        } else {
            None
        }
    })?;

    let mut leader_color = match leader_id {
        CollaboratorId::PeerId(leader_peer_id) => {
            let leader = GlobalAnyActiveCall::try_global(cx)?
                .0
                .remote_participant_for_peer_id(leader_peer_id, cx)?;

            cx.theme()
                .players()
                .color_for_participant(leader.participant_index.0)
                .cursor
        }
        CollaboratorId::Agent => cx.theme().players().agent().cursor,
    };
    leader_color.fade_out(0.3);
    Some(
        div()
            .absolute()
            .size_full()
            .left_0()
            .top_0()
            .border_2()
            .border_color(leader_color),
    )
}
