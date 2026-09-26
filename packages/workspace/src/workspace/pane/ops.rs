use super::*;
impl Workspace {
    // pub fn focus_center_pane
    pub fn focus_center_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.active_item(cx) {
            item.item_focus_handle(cx).focus(window, cx);
        } else {
            log::error!("Could not find a focus target when switching focus to the center panes",);
        }
    }
    // fn add_pane                        // private
    fn add_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Pane> {
        let pane = cx.new(|cx| {
            let mut pane = Pane::new(
                self.weak_handle(),
                self.project.clone(),
                self.pane_history_timestamp.clone(),
                None,
                NewFile.boxed_clone(),
                true,
                window,
                cx,
            );
            pane.set_can_split(Some(Arc::new(|_, _, _, _| true)));
            pane
        });
        cx.subscribe_in(&pane, window, Self::handle_pane_event)
            .detach();
        self.panes.push(pane.clone());

        window.focus(&pane.focus_handle(cx), cx);

        cx.emit(Event::PaneAdded(pane.clone()));
        pane
    }
    // pub fn swap_pane_in_direction
    pub fn swap_pane_in_direction(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        if let Some(to) = self.find_pane_in_direction(direction, cx) {
            self.center.swap(&self.active_pane, &to, cx);
            cx.notify();
        }
    }
    // pub fn move_pane_to_border
    pub fn move_pane_to_border(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        if self
            .center
            .move_to_border(&self.active_pane, direction, cx)
            .unwrap()
        {
            cx.notify();
        }
    }
    // pub fn split_pane
    pub fn split_pane(
        &mut self,
        pane_to_split: Entity<Pane>,
        split_direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Pane> {
        let new_pane = self.add_pane(window, cx);
        self.center
            .split(&pane_to_split, &new_pane, split_direction, cx);
        cx.notify();
        new_pane
    }
    // pub fn split_and_move
    pub fn split_and_move(
        &mut self,
        pane: Entity<Pane>,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = pane.update(cx, |pane, cx| pane.take_active_item(window, cx)) else {
            return;
        };
        let new_pane = self.add_pane(window, cx);
        new_pane.update(cx, |pane, cx| {
            pane.add_item(item, true, true, None, window, cx)
        });
        self.center.split(&pane, &new_pane, direction, cx);
        cx.notify();
    }
    // pub fn split_and_clone
    pub fn split_and_clone(
        &mut self,
        pane: Entity<Pane>,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Option<Entity<Pane>>> {
        let Some(item) = pane.read(cx).active_item() else {
            return Task::ready(None);
        };
        if !item.can_split(cx) {
            return Task::ready(None);
        }
        let task = item.clone_on_split(self.database_id(), window, cx);
        cx.spawn_in(window, async move |this, cx| {
            if let Some(clone) = task.await {
                this.update_in(cx, |this, window, cx| {
                    let new_pane = this.add_pane(window, cx);
                    let nav_history = pane.read(cx).fork_nav_history();
                    new_pane.update(cx, |pane, cx| {
                        pane.set_nav_history(nav_history, cx);
                        pane.add_item(clone, true, true, None, window, cx)
                    });
                    this.center.split(&pane, &new_pane, direction, cx);
                    cx.notify();
                    new_pane
                })
                .ok()
            } else {
                None
            }
        })
    }
    // pub fn join_all_panes
    pub fn join_all_panes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_item = self.active_pane.read(cx).active_item();
        for pane in &self.panes {
            join_pane_into_active(&self.active_pane, pane, window, cx);
        }
        if let Some(active_item) = active_item {
            self.activate_item(active_item.as_ref(), true, true, window, cx);
        }
        cx.notify();
    }
    // pub fn join_pane_into_next
    pub fn join_pane_into_next(
        &mut self,
        pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_pane = self
            .find_pane_in_direction(SplitDirection::Right, cx)
            .or_else(|| self.find_pane_in_direction(SplitDirection::Down, cx))
            .or_else(|| self.find_pane_in_direction(SplitDirection::Left, cx))
            .or_else(|| self.find_pane_in_direction(SplitDirection::Up, cx));
        let Some(next_pane) = next_pane else {
            return;
        };
        move_all_items(&pane, &next_pane, window, cx);
        cx.notify();
    }
    // fn remove_pane                     // private
    fn remove_pane(
        &mut self,
        pane: Entity<Pane>,
        focus_on: Option<Entity<Pane>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.center.remove(&pane, cx).unwrap() {
            if self
                .maximized_pane
                .as_ref()
                .and_then(|weak| weak.upgrade())
                .as_ref()
                == Some(&pane)
            {
                self.maximized_pane = None;
            }
            self.force_remove_pane(&pane, &focus_on, window, cx);
            self.unfollow_in_pane(&pane, window, cx);
            self.last_leaders_by_pane.remove(&pane.downgrade());
            for removed_item in pane.read(cx).items() {
                self.panes_by_item.remove(&removed_item.item_id());
            }

            cx.notify();
        } else {
            self.active_item_path_changed(true, window, cx);
        }
        cx.emit(Event::PaneRemoved);
    }
    //
    fn remove_panes(&mut self, member: Member, window: &mut Window, cx: &mut Context<Workspace>) {
        match member {
            Member::Axis(PaneAxis { members, .. }) => {
                for child in members.iter() {
                    self.remove_panes(child.clone(), window, cx)
                }
            }
            Member::Pane(pane) => {
                self.force_remove_pane(&pane, &None, window, cx);
            }
        }
    }
    //
    fn force_remove_pane(
        &mut self,
        pane: &Entity<Pane>,
        focus_on: &Option<Entity<Pane>>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let removing_active_pane = self.active_pane() == pane;
        self.panes.retain(|p| p != pane);
        if let Some(focus_on) = focus_on {
            if removing_active_pane {
                self.set_active_pane(focus_on, window, cx);
            }
            focus_on.update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx));
        } else if removing_active_pane {
            let fallback_pane = self.panes.last().unwrap().clone();
            self.set_active_pane(&fallback_pane, window, cx);
            if !self.has_active_modal(window, cx) {
                fallback_pane.update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx));
            }
        }
        if self.last_active_center_pane == Some(pane.downgrade()) {
            self.last_active_center_pane = None;
        }
        cx.notify();
    }
    // pub fn resize_pane
    pub fn resize_pane(
        &mut self,
        axis: gpui::Axis,
        amount: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let docks = self.all_docks();
        let active_dock = docks
            .into_iter()
            .find(|dock| dock.focus_handle(cx).contains_focused(window, cx));

        if let Some(dock_entity) = active_dock {
            let dock = dock_entity.read(cx);
            let Some(panel_size) = self.dock_size(&dock, window, cx) else {
                return;
            };
            match dock.position() {
                DockPosition::Left => self.resize_left_dock(panel_size + amount, window, cx),
                DockPosition::Bottom => self.resize_bottom_dock(panel_size + amount, window, cx),
                DockPosition::Right => self.resize_right_dock(panel_size + amount, window, cx),
            }
        } else {
            self.center
                .resize(&self.active_pane, axis, amount, &self.bounds, cx);
        }
        cx.notify();
    }
    // pub fn reset_pane_sizes
    pub fn reset_pane_sizes(&mut self, cx: &mut Context<Self>) {
        self.center.reset_pane_sizes(cx);
        cx.notify();
    }

    // pub fn adjacent_pane
    pub fn adjacent_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Pane> {
        self.adjacent_pane_of(&self.active_pane.clone(), window, cx)
    }
    // pub fn adjacent_pane_of
    pub fn adjacent_pane_of(
        &mut self,
        origin: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Pane> {
        self.center
            .find_pane_in_direction(origin, SplitDirection::Right, cx)
            .cloned()
            .unwrap_or_else(|| self.split_pane(origin.clone(), SplitDirection::Right, window, cx))
    }
    // │   └── adjust_padding
    fn adjust_padding(padding: Option<f32>) -> f32 {
        padding
            .unwrap_or(CenteredPaddingSettings::default().0)
            .clamp(
                CenteredPaddingSettings::MIN_PADDING,
                CenteredPaddingSettings::MAX_PADDING,
            )
    }
    // │   ├── swap_pane_in_direction / move_pane_to_border / resize_pane / reset_pane_sizes
    // │   ├── adjacent_pane / adjacent_pane_of
}
// join_pane_into_active
pub(crate) fn join_pane_into_active(
    active_pane: &Entity<Pane>,
    pane: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) {
    if pane == active_pane {
    } else if pane.read(cx).items_len() == 0 {
        pane.update(cx, |_, cx| {
            cx.emit(pane::Event::Remove {
                focus_on_pane: None,
            });
        })
    } else {
        move_all_items(pane, active_pane, window, cx);
    }
}
// / move_all_items
pub(crate) fn move_all_items(
    from_pane: &Entity<Pane>,
    to_pane: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) {
    let destination_is_different = from_pane != to_pane;
    let mut moved_items = 0;
    for (item_ix, item_handle) in from_pane
        .read(cx)
        .items()
        .enumerate()
        .map(|(ix, item)| (ix, item.clone()))
        .collect::<Vec<_>>()
    {
        let ix = item_ix - moved_items;
        if destination_is_different {
            // Close item from previous pane
            from_pane.update(cx, |source, cx| {
                source.remove_item_and_focus_on_pane(ix, false, to_pane.clone(), window, cx);
            });
            moved_items += 1;
        }

        // This automatically removes duplicate items in the pane
        to_pane.update(cx, |destination, cx| {
            destination.add_item(item_handle, true, true, None, window, cx);
            window.focus(&destination.focus_handle(cx), cx)
        });
    }
}
// move_item
pub fn move_item(
    source: &Entity<Pane>,
    destination: &Entity<Pane>,
    item_id_to_move: EntityId,
    destination_index: usize,
    activate: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((item_ix, item_handle)) = source
        .read(cx)
        .items()
        .enumerate()
        .find(|(_, item_handle)| item_handle.item_id() == item_id_to_move)
        .map(|(ix, item)| (ix, item.clone()))
    else {
        // Tab was closed during drag
        return;
    };

    if source != destination {
        // Close item from previous pane
        source.update(cx, |source, cx| {
            source.remove_item_and_focus_on_pane(item_ix, false, destination.clone(), window, cx);
        });
    }

    // This automatically removes duplicate items in the pane
    destination.update(cx, |destination, cx| {
        destination.add_item_inner(
            item_handle,
            activate,
            activate,
            activate,
            Some(destination_index),
            window,
            cx,
        );
        if activate {
            window.focus(&destination.focus_handle(cx), cx)
        }
    });
}
// move_active_item
pub fn move_active_item(
    source: &Entity<Pane>,
    destination: &Entity<Pane>,
    focus_destination: bool,
    close_if_empty: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if source == destination {
        return;
    }
    let Some(active_item) = source.read(cx).active_item() else {
        return;
    };
    source.update(cx, |source_pane, cx| {
        let item_id = active_item.item_id();
        source_pane.remove_item(item_id, false, close_if_empty, window, cx);
        destination.update(cx, |target_pane, cx| {
            target_pane.add_item(
                active_item,
                focus_destination,
                focus_destination,
                Some(target_pane.items_len()),
                window,
                cx,
            );
        });
    });
}
// clone_active_item
pub fn clone_active_item(
    workspace_id: Option<WorkspaceId>,
    source: &Entity<Pane>,
    destination: &Entity<Pane>,
    focus_destination: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if source == destination {
        return;
    }
    let Some(active_item) = source.read(cx).active_item() else {
        return;
    };
    if !active_item.can_split(cx) {
        return;
    }
    let destination = destination.downgrade();
    let task = active_item.clone_on_split(workspace_id, window, cx);
    window
        .spawn(cx, async move |cx| {
            let Some(clone) = task.await else {
                return;
            };
            destination
                .update_in(cx, |target_pane, window, cx| {
                    target_pane.add_item(
                        clone,
                        focus_destination,
                        focus_destination,
                        Some(target_pane.items_len()),
                        window,
                        cx,
                    );
                })
                .log_err();
        })
        .detach();
}
