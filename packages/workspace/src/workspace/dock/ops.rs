use super::*;

impl Workspace {
    // set_bottom_dock_layout     // 写 settings + serialize
    pub fn set_bottom_dock_layout(
        &mut self,
        layout: BottomDockLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fs = self.project().read(cx).fs();
        settings::update_settings_file(fs.clone(), cx, move |content, _cx| {
            content.workspace.bottom_dock_layout = Some(layout);
        });

        cx.notify();
        self.serialize_workspace(window, cx);
    }
    // pub fn toggle_dock
    pub fn toggle_dock(
        &mut self,
        dock_side: DockPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut focus_center = false;
        let mut reveal_dock = false;

        let other_is_zoomed = self.zoomed.is_some() && self.zoomed_position != Some(dock_side);
        let was_visible = self.is_dock_at_position_open(dock_side, cx) && !other_is_zoomed;

        if let Some(panel) = self.dock_at_position(dock_side).read(cx).active_panel() {
            telemetry::event!(
                "Panel Button Clicked",
                name = panel.persistent_name(),
                toggle_state = !was_visible
            );
        }
        if was_visible {
            self.save_open_dock_positions(cx);
        }

        let dock = self.dock_at_position(dock_side);
        dock.update(cx, |dock, cx| {
            dock.set_open(!was_visible, window, cx);

            if dock.active_panel().is_none() {
                let Some(panel_ix) = dock
                    .first_enabled_panel_idx(cx)
                    .log_with_level(log::Level::Info)
                else {
                    return;
                };
                dock.activate_panel(panel_ix, window, cx);
            }

            if let Some(active_panel) = dock.active_panel() {
                if was_visible {
                    if active_panel
                        .panel_focus_handle(cx)
                        .contains_focused(window, cx)
                    {
                        focus_center = true;
                    }
                } else {
                    let focus_handle = active_panel.activation_focus_handle(cx);
                    window.focus(&focus_handle, cx);
                    reveal_dock = true;
                }
            }
        });

        if reveal_dock {
            self.dismiss_zoomed_items_to_reveal(Some(dock_side), window, cx);
        }

        if focus_center {
            self.active_pane
                .update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx))
        }

        cx.notify();
        self.serialize_workspace(window, cx);
    }
    // fn close_active_dock               // private
    pub(crate) fn close_active_dock(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(dock) = self.active_dock(window, cx).cloned() {
            self.save_open_dock_positions(cx);
            dock.update(cx, |dock, cx| {
                dock.set_open(false, window, cx);
            });
            return true;
        }
        false
    }
    // pub fn close_all_docks
    pub fn close_all_docks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_open_dock_positions(cx);
        for dock in self.all_docks() {
            dock.update(cx, |dock, cx| {
                dock.set_open(false, window, cx);
            });
        }

        cx.focus_self(window);
        cx.notify();
        self.serialize_workspace(window, cx);
    }

    // save_open_dock_positions        // private
    /// Saves the positions of currently open docks.
    ///
    /// Updates `last_open_dock_positions` with positions of all currently open
    /// docks, to later be restored by the 'Toggle All Docks' action.
    pub(crate) fn save_open_dock_positions(&mut self, cx: &mut Context<Self>) {
        let open_dock_positions = self.get_open_dock_positions(cx);
        if !open_dock_positions.is_empty() {
            self.last_open_dock_positions = open_dock_positions;
        }
    }
    // toggle_all_docks                // private
    /// Toggles all docks between open and closed states.
    ///
    /// If any docks are open, closes all and remembers their positions. If all
    /// docks are closed, restores the last remembered dock configuration.
    pub(crate) fn toggle_all_docks(
        &mut self,
        _: &ToggleAllDocks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open_dock_positions = self.get_open_dock_positions(cx);

        if !open_dock_positions.is_empty() {
            self.close_all_docks(window, cx);
        } else if !self.last_open_dock_positions.is_empty() {
            self.restore_last_open_docks(window, cx);
        }
    }
    // restore_last_open_docks         // private
    /// Reopens docks from the most recently remembered configuration.
    ///
    /// Opens all docks whose positions are stored in `last_open_dock_positions`
    /// and clears the stored positions.
    pub(super) fn restore_last_open_docks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let positions_to_open = std::mem::take(&mut self.last_open_dock_positions);

        for position in positions_to_open {
            let dock = self.dock_at_position(position);
            dock.update(cx, |dock, cx| dock.set_open(true, window, cx));
        }

        cx.focus_self(window);
        cx.notify();
        self.serialize_workspace(window, cx);
    }
}
