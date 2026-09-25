use gpui::{Context, Entity};

use super::Workspace;
use crate::dock::Dock;

impl Workspace {
    // fn dismiss_zoomed_items_to_reveal
    fn dismiss_zoomed_items_to_reveal(
        &mut self,
        dock_to_reveal: Option<DockPosition>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If a center pane is zoomed, unzoom it.
        for pane in &self.panes {
            if pane != &self.active_pane || dock_to_reveal.is_some() {
                pane.update(cx, |pane, cx| pane.set_zoomed(false, cx));
            }
        }

        // If another dock is zoomed, hide it.
        let mut focus_center = false;
        for dock in self.all_docks() {
            dock.update(cx, |dock, cx| {
                if Some(dock.position()) != dock_to_reveal
                    && let Some(panel) = dock.active_panel()
                    && panel.is_zoomed(window, cx)
                {
                    focus_center |= panel.panel_focus_handle(cx).contains_focused(window, cx);
                    dock.set_open(false, window, cx);
                }
            });
        }

        if focus_center {
            self.active_pane
                .update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx))
        }

        if self.zoomed_position != dock_to_reveal {
            self.zoomed = None;
            self.zoomed_position = None;
            cx.emit(Event::ZoomChanged);
        }

        cx.notify();
    }


    /// Moves focus between the interactive controls within the title bar
    /// toolbar in response to arrow keys. Navigation is clamped to the title
    /// bar so arrows move between items and stop at the ends (ARIA toolbar
    /// semantics); Tab is still used to leave the toolbar.
    fn move_titlebar_item_focus(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = window.focused(cx);
        if forward {
            window.focus_next(cx);
        } else {
            window.focus_prev(cx);
        }
        let landed_in_titlebar = window
            .focused(cx)
            .is_some_and(|handle| self.titlebar_focus_handle.contains(&handle, window));
        // If Tab navigation wandered out of the toolbar, restore the previous
        // item so the ends of the toolbar act as stops rather than exits.
        if !landed_in_titlebar && let Some(previous) = previous {
            window.focus(&previous, cx);
        }
        cx.notify();
    }

    /// Moves focus to the next (or previous) visible window region. See
    /// [`FocusNextPart`].
    fn move_part_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let parts = self.focusable_parts(cx);
        if parts.is_empty() {
            return;
        }
        let current = parts
            .iter()
            .position(|part| part.contains_focused(window, cx));
        let next_index = match current {
            Some(index) if forward => (index + 1) % parts.len(),
            Some(index) => (index + parts.len() - 1) % parts.len(),
            None => 0,
        };
        let part = &parts[next_index];
        match &part.behavior {
            PartBehavior::Toolbar => {
                // The ARIA toolbar pattern requires focus to rest on the first
                // control, not the container. Focusing the tab-group container
                // and advancing descends into its first control; if the toolbar
                // has no focusable control, restore focus to the container so
                // navigation doesn't escape into an unrelated region.
                let container = part.container.clone();
                window.focus(&container, cx);
                window.focus_next(cx);
                let landed_inside = window
                    .focused(cx)
                    .is_some_and(|handle| container.contains(&handle, window));
                if !landed_inside {
                    window.focus(&container, cx);
                }
            }
            PartBehavior::Landmark { content } => {
                // Only redirect to the (otherwise non-focusable) wrapper when a
                // screen reader is active, so it is announced as a landmark.
                // Without a screen reader, focus the interactive content so
                // sighted keyboard users land somewhere usable, unchanged from
                // before this feature existed.
                if window.is_a11y_active() {
                    window.focus(&part.container, cx);
                } else {
                    window.focus(content, cx);
                }
            }
        }
        cx.notify();
    }

    //
    pub fn toggle_editor_zoom(
        &mut self,
        _: &ToggleEditorZoom,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.zoomed.is_some() {
            self.active_pane.update(cx, |pane, cx| {
                pane.set_zoomed(false, cx);
            });
            self.zoomed = None;
            self.zoomed_position = None;
            cx.emit(Event::ZoomChanged);
        }

        if let Some(maximized) = self.maximized_pane.take() {
            if maximized.upgrade().as_ref() == Some(&self.active_pane) {
                cx.notify();
                return;
            }
        }

        self.maximized_pane = Some(self.active_pane.downgrade());
        window.focus(&self.active_pane.focus_handle(cx), cx);
        cx.notify();
    }
    //
    pub fn is_pane_maximized(&self) -> bool { self.maximized_pane.is_some() }
    pub(crate) fn handle_panel_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_deferred_saves(window, cx);
        self.update_active_view_for_followers(window, cx);
    }
    pub fn move_focused_panel_to_next_position(
        &mut self,
        _: &MoveFocusedPanelToNextPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let docks = self.all_docks();
        let active_dock = docks
            .into_iter()
            .find(|dock| dock.focus_handle(cx).contains_focused(window, cx));

        if let Some(dock) = active_dock {
            dock.update(cx, |dock, cx| {
                let active_panel = dock
                    .active_panel()
                    .filter(|panel| panel.panel_focus_handle(cx).contains_focused(window, cx));

                if let Some(panel) = active_panel {
                    panel.move_to_next_position(window, cx);
                }
            })
        }
    }

    pub fn activate_pane_in_direction(
        &mut self,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut App,
    ) {
        use ActivateInDirectionTarget as Target;
        enum Origin {
            Sidebar,
            LeftDock,
            RightDock,
            BottomDock,
            Center,
        }

        let origin: Origin = if self
            .sidebar_focus_handle
            .as_ref()
            .is_some_and(|h| h.contains_focused(window, cx))
        {
            Origin::Sidebar
        } else {
            [
                (&self.left_dock, Origin::LeftDock),
                (&self.right_dock, Origin::RightDock),
                (&self.bottom_dock, Origin::BottomDock),
            ]
            .into_iter()
            .find_map(|(dock, origin)| {
                if dock.focus_handle(cx).contains_focused(window, cx) && dock.read(cx).is_open() {
                    Some(origin)
                } else {
                    None
                }
            })
            .unwrap_or(Origin::Center)
        };

        let get_last_active_pane = || {
            let pane = self
                .last_active_center_pane
                .clone()
                .unwrap_or_else(|| {
                    self.panes
                        .first()
                        .expect("There must be an active pane")
                        .downgrade()
                })
                .upgrade()?;
            (pane.read(cx).items_len() != 0).then_some(pane)
        };

        let try_dock =
            |dock: &Entity<Dock>| dock.read(cx).is_open().then(|| Target::Dock(dock.clone()));

        let sidebar_target = self
            .sidebar_focus_handle
            .as_ref()
            .map(|h| Target::Sidebar(h.clone()));

        let sidebar_on_right = self
            .multi_workspace
            .as_ref()
            .and_then(|mw| mw.upgrade())
            .map_or(false, |mw| {
                mw.read(cx).sidebar_side(cx) == SidebarSide::Right
            });

        let away_from_sidebar = if sidebar_on_right {
            SplitDirection::Left
        } else {
            SplitDirection::Right
        };

        let (near_dock, far_dock) = if sidebar_on_right {
            (&self.right_dock, &self.left_dock)
        } else {
            (&self.left_dock, &self.right_dock)
        };

        let target = match (origin, direction) {
            (Origin::Sidebar, dir) if dir == away_from_sidebar => try_dock(near_dock)
                .or_else(|| get_last_active_pane().map(Target::Pane))
                .or_else(|| try_dock(&self.bottom_dock))
                .or_else(|| try_dock(far_dock)),

            (Origin::Sidebar, _) => None,

            // We're in the center, so we first try to go to a different pane,
            // otherwise try to go to a dock.
            (Origin::Center, direction) => {
                if let Some(pane) = self.find_pane_in_direction(direction, cx) {
                    Some(Target::Pane(pane))
                } else {
                    match direction {
                        SplitDirection::Up => None,
                        SplitDirection::Down => try_dock(&self.bottom_dock),
                        SplitDirection::Left => {
                            let dock_target = try_dock(&self.left_dock);
                            if sidebar_on_right {
                                dock_target
                            } else {
                                dock_target.or(sidebar_target)
                            }
                        }
                        SplitDirection::Right => {
                            let dock_target = try_dock(&self.right_dock);
                            if sidebar_on_right {
                                dock_target.or(sidebar_target)
                            } else {
                                dock_target
                            }
                        }
                    }
                }
            }

            (Origin::LeftDock, SplitDirection::Right) => {
                if let Some(last_active_pane) = get_last_active_pane() {
                    Some(Target::Pane(last_active_pane))
                } else {
                    try_dock(&self.bottom_dock).or_else(|| try_dock(&self.right_dock))
                }
            }

            (Origin::LeftDock, SplitDirection::Left) => {
                if sidebar_on_right {
                    None
                } else {
                    sidebar_target
                }
            }

            (Origin::LeftDock, SplitDirection::Down)
            | (Origin::RightDock, SplitDirection::Down) => try_dock(&self.bottom_dock),

            (Origin::BottomDock, SplitDirection::Up) => get_last_active_pane().map(Target::Pane),
            (Origin::BottomDock, SplitDirection::Left) => {
                let dock_target = try_dock(&self.left_dock);
                if sidebar_on_right {
                    dock_target
                } else {
                    dock_target.or(sidebar_target)
                }
            }
            (Origin::BottomDock, SplitDirection::Right) => {
                let dock_target = try_dock(&self.right_dock);
                if sidebar_on_right {
                    dock_target.or(sidebar_target)
                } else {
                    dock_target
                }
            }

            (Origin::RightDock, SplitDirection::Left) => {
                if let Some(last_active_pane) = get_last_active_pane() {
                    Some(Target::Pane(last_active_pane))
                } else {
                    try_dock(&self.bottom_dock).or_else(|| try_dock(&self.left_dock))
                }
            }

            (Origin::RightDock, SplitDirection::Right) => {
                if sidebar_on_right {
                    sidebar_target
                } else {
                    None
                }
            }

            _ => None,
        };

        match target {
            Some(ActivateInDirectionTarget::Pane(pane)) => {
                let pane = pane.read(cx);
                if let Some(item) = pane.active_item() {
                    item.item_focus_handle(cx).focus(window, cx);
                } else {
                    log::error!(
                        "Could not find a focus target when in switching focus in {direction} direction for a pane",
                    );
                }
            }
            Some(ActivateInDirectionTarget::Dock(dock)) => {
                // Defer this to avoid a panic when the dock's active panel is already on the stack.
                window.defer(cx, move |window, cx| {
                    let dock = dock.read(cx);
                    if let Some(panel) = dock.active_panel() {
                        panel.activation_focus_handle(cx).focus(window, cx);
                    } else {
                        log::error!("Could not find a focus target when in switching focus in {direction} direction for a {:?} dock", dock.position());
                    }
                })
            }
            Some(ActivateInDirectionTarget::Sidebar(focus_handle)) => {
                focus_handle.focus(window, cx);
            }
            None => {}
        }
    }
    fn split_pane(cx: &mut VisualTestContext, workspace: &Entity<Workspace>) -> Entity<Pane> {
        workspace.update_in(cx, |workspace, window, cx| {
            workspace.split_pane(
                workspace.active_pane().clone(),
                SplitDirection::Right,
                window,
                cx,
            )
        })
    }

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
    }
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
    pub fn find_pane_in_direction(
        &mut self,
        direction: SplitDirection,
        cx: &App,
    ) -> Option<Entity<Pane>> {
        self.center
            .find_pane_in_direction(&self.active_pane, direction, cx)
            .cloned()
    }
    pub fn activate_next_pane(&mut self, window: &mut Window, cx: &mut App) {
        let panes = self.center.panes();
        if let Some(ix) = panes.iter().position(|pane| **pane == self.active_pane) {
            let next_ix = (ix + 1) % panes.len();
            let next_pane = panes[next_ix].clone();
            window.focus(&next_pane.focus_handle(cx), cx);
        }
    }
    pub fn activate_previous_pane(&mut self, window: &mut Window, cx: &mut App) {
        let panes = self.center.panes();
        if let Some(ix) = panes.iter().position(|pane| **pane == self.active_pane) {
            let prev_ix = cmp::min(ix.wrapping_sub(1), panes.len() - 1);
            let prev_pane = panes[prev_ix].clone();
            window.focus(&prev_pane.focus_handle(cx), cx);
        }
    }

    pub fn activate_last_pane(&mut self, window: &mut Window, cx: &mut App) {
        let last_pane = self.center.last_pane();
        window.focus(&last_pane.focus_handle(cx), cx);
    }
    fn activate_pane_at_index(
        &mut self,
        action: &ActivatePane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panes = self.center.panes();
        if let Some(pane) = panes.get(action.0).map(|p| (*p).clone()) {
            window.focus(&pane.focus_handle(cx), cx);
        } else {
            self.split_and_clone(self.active_pane.clone(), SplitDirection::Right, window, cx)
                .detach();
        }
    }
    fn move_item_to_pane_at_index(
        &mut self,
        action: &MoveItemToPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panes = self.center.panes();
        let destination = match panes.get(action.destination) {
            Some(&destination) => destination.clone(),
            None => {
                if !action.clone && self.active_pane.read(cx).items_len() < 2 {
                    return;
                }
                let direction = SplitDirection::Right;
                let split_off_pane = self
                    .find_pane_in_direction(direction, cx)
                    .unwrap_or_else(|| self.active_pane.clone());
                let new_pane = self.add_pane(window, cx);
                self.center.split(&split_off_pane, &new_pane, direction, cx);
                new_pane
            }
        };

        if action.clone {
            if self
                .active_pane
                .read(cx)
                .active_item()
                .is_some_and(|item| item.can_split(cx))
            {
                clone_active_item(
                    self.database_id(),
                    &self.active_pane,
                    &destination,
                    action.focus,
                    window,
                    cx,
                );
                return;
            }
        }
        move_active_item(
            &self.active_pane,
            &destination,
            action.focus,
            true,
            window,
            cx,
        )
    }
    pub fn move_item_to_pane_in_direction(
        &mut self,
        action: &MoveItemToPaneInDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let destination = match self.find_pane_in_direction(action.direction, cx) {
            Some(destination) => destination,
            None => {
                if !action.clone && self.active_pane.read(cx).items_len() < 2 {
                    return;
                }
                let new_pane = self.add_pane(window, cx);
                self.center
                    .split(&self.active_pane, &new_pane, action.direction, cx);
                new_pane
            }
        };

        if action.clone {
            if self
                .active_pane
                .read(cx)
                .active_item()
                .is_some_and(|item| item.can_split(cx))
            {
                clone_active_item(
                    self.database_id(),
                    &self.active_pane,
                    &destination,
                    action.focus,
                    window,
                    cx,
                );
                return;
            }
        }
        move_active_item(
            &self.active_pane,
            &destination,
            action.focus,
            true,
            window,
            cx,
        );
    }

    pub fn swap_pane_in_direction(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        if let Some(to) = self.find_pane_in_direction(direction, cx) {
            self.center.swap(&self.active_pane, &to, cx);
            cx.notify();
        }
    }

    pub fn move_pane_to_border(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        if self
            .center
            .move_to_border(&self.active_pane, direction, cx)
            .unwrap()
        {
            cx.notify();
        }
    }

    pub fn adjacent_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Pane> {
        self.adjacent_pane_of(&self.active_pane.clone(), window, cx)
    }

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

    pub fn reset_pane_sizes(&mut self, cx: &mut Context<Self>) {
        self.center.reset_pane_sizes(cx);
        cx.notify();
    }
    pub(super) fn handle_pane_event(
        &mut self,
        pane: &Entity<Pane>,
        event: &pane::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut serialize_workspace = true;
        match event {
            pane::Event::AddItem { item } => {
                item.added_to_pane(self, pane.clone(), window, cx);
                cx.emit(Event::ItemAdded {
                    item: item.boxed_clone(),
                });
            }
            pane::Event::Split { direction, mode } => {
                match mode {
                    SplitMode::ClonePane => {
                        self.split_and_clone(pane.clone(), *direction, window, cx)
                            .detach();
                    }
                    SplitMode::EmptyPane => {
                        self.split_pane(pane.clone(), *direction, window, cx);
                    }
                    SplitMode::MovePane => {
                        self.split_and_move(pane.clone(), *direction, window, cx);
                    }
                };
            }
            pane::Event::JoinIntoNext => {
                self.join_pane_into_next(pane.clone(), window, cx);
            }
            pane::Event::JoinAll => {
                self.join_all_panes(window, cx);
            }
            pane::Event::Remove { focus_on_pane } => {
                self.remove_pane(pane.clone(), focus_on_pane.clone(), window, cx);
            }
            pane::Event::ActivateItem {
                local,
                focus_changed,
            } => {
                window.invalidate_character_coordinates();

                pane.update(cx, |pane, _| {
                    pane.track_alternate_file_items();
                });
                if *local {
                    self.unfollow_in_pane(pane, window, cx);
                }
                serialize_workspace = *focus_changed || pane != self.active_pane();
                if pane == self.active_pane() {
                    self.active_item_path_changed(*focus_changed, window, cx);
                    self.update_active_view_for_followers(window, cx);
                } else if *local {
                    self.set_active_pane(pane, window, cx);
                }
            }
            pane::Event::UserSavedItem { item, save_intent } => {
                cx.emit(Event::UserSavedItem {
                    pane: pane.downgrade(),
                    item: item.boxed_clone(),
                    save_intent: *save_intent,
                });
                serialize_workspace = false;
            }
            pane::Event::ChangeItemTitle => {
                if *pane == self.active_pane {
                    self.active_item_path_changed(false, window, cx);
                    cx.notify();
                }
                serialize_workspace = false;
            }
            pane::Event::RemovedItem { item } => {
                cx.emit(Event::ActiveItemChanged);
                self.update_window_edited(window, cx);
                if let hash_map::Entry::Occupied(entry) = self.panes_by_item.entry(item.item_id())
                    && entry.get().entity_id() == pane.entity_id()
                {
                    entry.remove();
                }
                cx.emit(Event::ItemRemoved {
                    item_id: item.item_id(),
                });
            }
            pane::Event::Focus => {
                window.invalidate_character_coordinates();
                self.handle_pane_focused(pane.clone(), window, cx);
            }
            pane::Event::ZoomIn => {
                if *pane == self.active_pane {
                    self.maximized_pane = None;
                    pane.update(cx, |pane, cx| pane.set_zoomed(true, cx));
                    if pane.read(cx).has_focus(window, cx) {
                        self.zoomed = Some(pane.downgrade().into());
                        self.zoomed_position = None;
                        cx.emit(Event::ZoomChanged);
                    }
                    cx.notify();
                }
            }
            pane::Event::ZoomOut => {
                pane.update(cx, |pane, cx| pane.set_zoomed(false, cx));
                if self.zoomed_position.is_none() {
                    self.zoomed = None;
                    cx.emit(Event::ZoomChanged);
                }
                cx.notify();
            }
            pane::Event::ItemPinned | pane::Event::ItemUnpinned => {}
        }

        if serialize_workspace {
            self.serialize_workspace(window, cx);
        }
    }
    pub(crate) fn handle_pane_focused(
        &mut self,
        pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.flush_deferred_saves(window, cx);

        // This is explicitly hoisted out of the following check for pane identity as
        // terminal panel panes are not registered as a center panes.
        self.status_bar.update(cx, |status_bar, cx| {
            status_bar.set_active_pane(&pane, window, cx);
        });
        if self.active_pane != pane {
            self.set_active_pane(&pane, window, cx);
        }

        if self.last_active_center_pane.is_none() {
            self.last_active_center_pane = Some(pane.downgrade());
        }

        // If this pane is in a dock, preserve that dock when dismissing zoomed items.
        // This prevents the dock from closing when focus events fire during window activation.
        // We also preserve any dock whose active panel itself has focus — this covers
        // panels like AgentPanel that don't implement `pane()` but can still be zoomed.
        let dock_to_preserve = self.all_docks().iter().find_map(|dock| {
            let dock_read = dock.read(cx);
            if let Some(panel) = dock_read.active_panel() {
                if panel.pane(cx).is_some_and(|dock_pane| dock_pane == pane)
                    || panel.panel_focus_handle(cx).contains_focused(window, cx)
                {
                    return Some(dock_read.position());
                }
            }
            None
        });

        if let Some(maximized) = &self.maximized_pane {
            let is_center_pane = self.panes.contains(&pane);
            if is_center_pane && maximized.upgrade().as_ref() != Some(&pane) {
                self.maximized_pane = None;
            }
        }

        self.dismiss_zoomed_items_to_reveal(dock_to_preserve, window, cx);
        if pane.read(cx).is_zoomed() {
            self.zoomed = Some(pane.downgrade().into());
        } else {
            self.zoomed = None;
        }
        self.zoomed_position = None;
        cx.emit(Event::ZoomChanged);
        self.update_active_view_for_followers(window, cx);
        pane.update(cx, |pane, _| {
            pane.track_alternate_file_items();
        });

        cx.notify();
    }

    pub(super) fn set_active_pane(
        &mut self,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_pane = pane.clone();
        self.active_item_path_changed(true, window, cx);
        self.last_active_center_pane = Some(pane.downgrade());
    }

    pub(super) fn flush_deferred_saves(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let deferred = std::mem::take(&mut self.deferred_save_items);
        for weak_item in deferred {
            let Some(item) = weak_item.upgrade() else {
                continue;
            };
            // Skip if focus returned to this item
            let focus_handle = item.item_focus_handle(cx);
            if focus_handle.contains_focused(window, cx) {
                continue;
            }
            Pane::autosave_item(item.as_ref(), self.project.clone(), window, cx)
                .detach_and_log_err(cx);
        }
    }
    pub fn active_pane(&self) -> &Entity<Pane> { &self.active_pane }
    pub fn panes(&self) -> &[Entity<Pane>] { &self.panes }
    pub fn panes_mut(&mut self) -> &mut [Entity<Pane>] { &mut self.panes }
    pub fn focused_pane(&self, window: &Window, cx: &App) -> Entity<Pane> {
        for dock in self.all_docks() {
            if dock.focus_handle(cx).contains_focused(window, cx)
                && let Some(pane) = dock
                    .read(cx)
                    .active_panel()
                    .and_then(|panel| panel.pane(cx))
            {
                return pane;
            }
        }
        self.active_pane().clone()
    }
    pub fn bounding_box_for_pane(&self, pane: &Entity<Pane>) -> Option<Bounds<Pixels>> {
        self.center.bounding_box_for_pane(pane)
    }
    pub fn pane_for(&self, handle: &dyn ItemHandle) -> Option<Entity<Pane>> {
        self.pane_for_item_id(handle.item_id())
    }

    pub fn pane_for_item_id(&self, item_id: EntityId) -> Option<Entity<Pane>> {
        let weak_pane = self.panes_by_item.get(&item_id)?;
        weak_pane.upgrade()
    }

    pub fn pane_for_entity_id(&self, entity_id: EntityId) -> Option<Entity<Pane>> {
        self.panes
            .iter()
            .find(|pane| pane.entity_id() == entity_id)
            .cloned()
    }
}
