use super::*;
impl Workspace {
    // handle_agent_location_changed
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
    //
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

}
