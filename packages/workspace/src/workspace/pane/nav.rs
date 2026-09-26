use super::*;
impl Workspace {
    // fn activate_pane_at_index          // private，但主语是 pane 索引
    pub(crate) fn activate_pane_at_index(
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
    // pub fn activate_next_pane
    pub fn activate_next_pane(&mut self, window: &mut Window, cx: &mut App) {
        let panes = self.center.panes();
        if let Some(ix) = panes.iter().position(|pane| **pane == self.active_pane) {
            let next_ix = (ix + 1) % panes.len();
            let next_pane = panes[next_ix].clone();
            window.focus(&next_pane.focus_handle(cx), cx);
        }
    }
    // pub fn activate_previous_pane
    pub fn activate_previous_pane(&mut self, window: &mut Window, cx: &mut App) {
        let panes = self.center.panes();
        if let Some(ix) = panes.iter().position(|pane| **pane == self.active_pane) {
            let prev_ix = cmp::min(ix.wrapping_sub(1), panes.len() - 1);
            let prev_pane = panes[prev_ix].clone();
            window.focus(&prev_pane.focus_handle(cx), cx);
        }
    }
    // pub fn activate_last_pane
    pub fn activate_last_pane(&mut self, window: &mut Window, cx: &mut App) {
        let last_pane = self.center.last_pane();
        window.focus(&last_pane.focus_handle(cx), cx);
    }
    // pub fn activate_pane_in_direction
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
    // pub fn find_pane_in_direction
    pub fn find_pane_in_direction(
        &mut self,
        direction: SplitDirection,
        cx: &App,
    ) -> Option<Entity<Pane>> {
        self.center
            .find_pane_in_direction(&self.active_pane, direction, cx)
            .cloned()
    }
}

#[derive(Clone)]
pub(crate) enum ActivateInDirectionTarget {
    Pane(Entity<Pane>),
    Dock(Entity<Dock>),
    Sidebar(FocusHandle),
}
