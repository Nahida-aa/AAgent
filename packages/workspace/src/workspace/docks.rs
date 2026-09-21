use gpui::{Context, Entity, Window};

use super::Workspace;
use crate::dock::Dock;

impl Workspace {
    pub fn left_dock(&self) -> &Entity<Dock> { &self.left_dock }
    pub fn bottom_dock(&self) -> &Entity<Dock> { &self.bottom_dock }
    pub fn right_dock(&self) -> &Entity<Dock> { &self.right_dock }
    pub fn all_docks(&self) -> [&Entity<Dock>; 3] {
        [&self.left_dock, &self.bottom_dock, &self.right_dock]
    }
    pub fn dock_at_position(&self, position: DockPosition) -> &Entity<Dock> {
        match position {
            DockPosition::Left => &self.left_dock,
            DockPosition::Bottom => &self.bottom_dock,
            DockPosition::Right => &self.right_dock,
        }
    }

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
    fn close_active_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(dock) = self.active_dock(window, cx).cloned() {
            self.save_open_dock_positions(cx);
            dock.update(cx, |dock, cx| {
                dock.set_open(false, window, cx);
            });
            return true;
        }
        false
    }
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

    /// Saves the positions of currently open docks.
    ///
    /// Updates `last_open_dock_positions` with positions of all currently open
    /// docks, to later be restored by the 'Toggle All Docks' action.
    fn save_open_dock_positions(&mut self, cx: &mut Context<Self>) {
        let open_dock_positions = self.get_open_dock_positions(cx);
        if !open_dock_positions.is_empty() {
            self.last_open_dock_positions = open_dock_positions;
        }
    }
    pub(super) fn get_open_dock_positions(&self, cx: &Context<Self>) -> Vec<DockPosition> {
        self.all_docks()
            .into_iter()
            .filter_map(|dock| {
                let dock_ref = dock.read(cx);
                if dock_ref.is_open() {
                    Some(dock_ref.position())
                } else {
                    None
                }
            })
            .collect()
    }
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

    fn active_dock(&self, window: &Window, cx: &Context<Self>) -> Option<&Entity<Dock>> {
        self.all_docks().into_iter().find(|&dock| {
            dock.read(cx).is_open() && dock.focus_handle(cx).contains_focused(window, cx)
        })
    }
    /// Returns which dock currently has focus, or `None` if focus is in the
    /// center pane or elsewhere. Does NOT fall back to any global state.
    pub fn focused_dock_position(&self, window: &Window, cx: &App) -> Option<DockPosition> {
        [
            (DockPosition::Left, &self.left_dock),
            (DockPosition::Right, &self.right_dock),
            (DockPosition::Bottom, &self.bottom_dock),
        ]
        .into_iter()
        .find(|(_, dock)| {
            dock.read(cx).is_open() && dock.focus_handle(cx).contains_focused(window, cx)
        })
        .map(|(position, _)| position)
    }
    pub fn is_dock_at_position_open(&self, position: DockPosition, cx: &mut Context<Self>) -> bool {
        self.dock_at_position(position).read(cx).is_open()
    }

    pub fn capture_dock_state(&self, _window: &Window, cx: &App) -> DockStructure {
        let left_dock = self.left_dock.read(cx);
        let left_visible = left_dock.is_open();
        let left_active_panel = left_dock
            .active_panel()
            .map(|panel| panel.persistent_name().to_string());
        // `zoomed_position` is kept in sync with individual panel zoom state
        // by the dock code in `Dock::new` and `Dock::add_panel`.
        let left_dock_zoom = self.zoomed_position == Some(DockPosition::Left);

        let right_dock = self.right_dock.read(cx);
        let right_visible = right_dock.is_open();
        let right_active_panel = right_dock
            .active_panel()
            .map(|panel| panel.persistent_name().to_string());
        let right_dock_zoom = self.zoomed_position == Some(DockPosition::Right);

        let bottom_dock = self.bottom_dock.read(cx);
        let bottom_visible = bottom_dock.is_open();
        let bottom_active_panel = bottom_dock
            .active_panel()
            .map(|panel| panel.persistent_name().to_string());
        let bottom_dock_zoom = self.zoomed_position == Some(DockPosition::Bottom);

        DockStructure {
            left: DockData {
                visible: left_visible,
                active_panel: left_active_panel,
                zoom: left_dock_zoom,
            },
            right: DockData {
                visible: right_visible,
                active_panel: right_active_panel,
                zoom: right_dock_zoom,
            },
            bottom: DockData {
                visible: bottom_visible,
                active_panel: bottom_active_panel,
                zoom: bottom_dock_zoom,
            },
        }
    }

    pub fn set_dock_structure(
        &self,
        docks: DockStructure,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (dock, data) in [
            (&self.left_dock, docks.left),
            (&self.bottom_dock, docks.bottom),
            (&self.right_dock, docks.right),
        ] {
            dock.update(cx, |dock, cx| {
                dock.restore_serialized_state(data, window, cx);
            });
        }
    }

    pub fn finish_dock_restoration(&self, cx: &mut App) {
        for dock in [&self.left_dock, &self.bottom_dock, &self.right_dock] {
            dock.update(cx, |dock, _| {
                dock.finish_restoration();
            });
        }
    }

    pub(super) fn dock_size(&self, dock: &Dock, window: &Window, cx: &App) -> Option<Pixels> {
        let panel = dock.active_panel()?;
        let size_state = dock
            .stored_panel_size_state(panel.as_ref())
            .unwrap_or_default();
        let position = dock.position();

        let use_flex = panel.has_flexible_size(window, cx);

        if position.axis() == Axis::Horizontal
            && use_flex
            && let Some(flex) = size_state.flex.or_else(|| self.default_dock_flex(position))
        {
            let workspace_width = self.bounds.size.width;
            if workspace_width <= Pixels::ZERO {
                return None;
            }
            let flex = flex.max(0.001);
            let center_column_count = self.center_full_height_column_count();
            let opposite = self.opposite_dock_panel_and_size_state(position, window, cx);
            if let Some(opposite_flex) = opposite.as_ref().and_then(|(_, s)| s.flex) {
                let total_flex = flex + center_column_count + opposite_flex;
                return Some((flex / total_flex * workspace_width).max(RESIZE_HANDLE_SIZE));
            } else {
                let opposite_fixed = opposite
                    .map(|(panel, s)| s.size.unwrap_or_else(|| panel.default_size(window, cx)))
                    .unwrap_or_default();
                let available = (workspace_width - opposite_fixed).max(RESIZE_HANDLE_SIZE);
                return Some(
                    (flex / (flex + center_column_count) * available).max(RESIZE_HANDLE_SIZE),
                );
            }
        }

        Some(
            size_state
                .size
                .unwrap_or_else(|| panel.default_size(window, cx)),
        )
    }

    pub fn dock_flex_for_size(
        &self,
        position: DockPosition,
        size: Pixels,
        window: &Window,
        cx: &App,
    ) -> Option<f32> {
        if position.axis() != Axis::Horizontal {
            return None;
        }

        let workspace_width = self.bounds.size.width;
        if workspace_width <= Pixels::ZERO {
            return None;
        }

        let center_column_count = self.center_full_height_column_count();
        let opposite = self.opposite_dock_panel_and_size_state(position, window, cx);
        if let Some(opposite_flex) = opposite.as_ref().and_then(|(_, s)| s.flex) {
            let size = size.clamp(px(0.), workspace_width - px(1.));
            Some((size * (center_column_count + opposite_flex) / (workspace_width - size)).max(0.0))
        } else {
            let opposite_width = opposite
                .map(|(panel, s)| s.size.unwrap_or_else(|| panel.default_size(window, cx)))
                .unwrap_or_default();
            let available = (workspace_width - opposite_width).max(RESIZE_HANDLE_SIZE);
            let remaining = (available - size).max(px(1.));
            Some((size * center_column_count / remaining).max(0.0))
        }
    }

    fn opposite_dock_panel_and_size_state(
        &self,
        position: DockPosition,
        window: &Window,
        cx: &App,
    ) -> Option<(Arc<dyn PanelHandle>, PanelSizeState)> {
        let opposite_position = match position {
            DockPosition::Left => DockPosition::Right,
            DockPosition::Right => DockPosition::Left,
            DockPosition::Bottom => return None,
        };

        let opposite_dock = self.dock_at_position(opposite_position).read(cx);
        let panel = opposite_dock.visible_panel()?;
        let mut size_state = opposite_dock
            .stored_panel_size_state(panel.as_ref())
            .unwrap_or_default();
        if size_state.flex.is_none() && panel.has_flexible_size(window, cx) {
            size_state.flex = self.default_dock_flex(opposite_position);
        }
        Some((panel.clone(), size_state))
    }

    fn center_full_height_column_count(&self) -> f32 {
        self.center.full_height_column_count().max(1) as f32
    }

    pub fn default_dock_flex(&self, position: DockPosition) -> Option<f32> {
        if position.axis() != Axis::Horizontal {
            return None;
        }

        Some(1.0)
    }

    fn resize_dock(
        &mut self,
        dock_pos: DockPosition,
        new_size: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match dock_pos {
            DockPosition::Left => self.resize_left_dock(new_size, window, cx),
            DockPosition::Right => self.resize_right_dock(new_size, window, cx),
            DockPosition::Bottom => self.resize_bottom_dock(new_size, window, cx),
        }
    }

    pub(super) fn resize_left_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App) {
        let workspace_width = self.bounds.size.width;
        let mut size = new_size.min(workspace_width - RESIZE_HANDLE_SIZE);

        self.right_dock.read_with(cx, |right_dock, cx| {
            let right_dock_size = right_dock
                .stored_active_panel_size(window, cx)
                .unwrap_or(Pixels::ZERO);
            if right_dock_size + size > workspace_width {
                size = workspace_width - right_dock_size
            }
        });

        let flex_grow = self.dock_flex_for_size(DockPosition::Left, size, window, cx);
        self.left_dock.update(cx, |left_dock, cx| {
            left_dock.resize_panel_sizes(Some(size), flex_grow, window, cx);
        });
    }

    pub(super) fn resize_right_dock(
        &mut self,
        new_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let workspace_width = self.bounds.size.width;
        let mut size = new_size.min(workspace_width - RESIZE_HANDLE_SIZE);
        self.left_dock.read_with(cx, |left_dock, cx| {
            let left_dock_size = left_dock
                .stored_active_panel_size(window, cx)
                .unwrap_or(Pixels::ZERO);
            if left_dock_size + size > workspace_width {
                size = workspace_width - left_dock_size
            }
        });
        let flex_grow = self.dock_flex_for_size(DockPosition::Right, size, window, cx);
        self.right_dock.update(cx, |right_dock, cx| {
            right_dock.resize_panel_sizes(Some(size), flex_grow, window, cx);
        });
    }

    pub(super) fn resize_bottom_dock(
        &mut self,
        new_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = new_size.min(self.bounds.bottom() - RESIZE_HANDLE_SIZE - self.bounds.top());
        self.bottom_dock.update(cx, |bottom_dock, cx| {
            bottom_dock.resize_panel_sizes(Some(size), None, window, cx);
        });
    }

    pub(super) fn render_dock(
        &self,
        position: DockPosition,
        dock: &Entity<Dock>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Stateful<Div>> {
        if self.zoomed_position == Some(position) {
            return None;
        }

        let leader_border = dock.read(cx).active_panel().and_then(|panel| {
            let pane = panel.pane(cx)?;
            let follower_states = &self.follower_states;
            leader_border_for_pane(follower_states, &pane, window, cx)
        });

        // Expose each open dock as a landmark region so assistive technology
        // can navigate to it, and so region navigation announces it. While a
        // screen reader is active the wrapper is also the focus target for
        // region navigation (it carries the landmark role and label), so
        // focusing it announces the region instead of falling back to the whole
        // window. We only make it focusable in that case so it never adds a
        // hitbox or intercepts mouse focus for other users.
        let (dock_element_id, dock_label) = match position {
            DockPosition::Left => ("left-dock", "Left dock"),
            DockPosition::Right => ("right-dock", "Right dock"),
            DockPosition::Bottom => ("bottom-dock", "Bottom dock"),
        };
        let dock_is_open = dock.read(cx).is_open();
        let a11y_active = window.is_a11y_active();

        let mut container = div()
            .id(dock_element_id)
            .when(dock_is_open, |this| {
                this.role(gpui::Role::Complementary)
                    .aria_label(dock_label)
                    .when(a11y_active, |this| {
                        this.track_focus(self.region_focus_handles.dock(position))
                    })
            })
            .flex()
            .overflow_hidden()
            .flex_none()
            .child(dock.clone())
            .children(leader_border);

        // Apply sizing only when the dock is open. When closed the dock is still
        // included in the element tree so its focus handle remains mounted — without
        // this, toggle_panel_focus cannot focus the panel when the dock is closed.
        let dock = dock.read(cx);
        if let Some(panel) = dock.visible_panel() {
            let size_state = dock.stored_panel_size_state(panel.as_ref());
            let min_size = panel.min_size(window, cx);
            if position.axis() == Axis::Horizontal {
                let use_flexible = panel.has_flexible_size(window, cx);
                let flex_grow = if use_flexible {
                    size_state
                        .and_then(|state| state.flex)
                        .or_else(|| self.default_dock_flex(position))
                } else {
                    None
                };
                if let Some(grow) = flex_grow {
                    let grow = (grow / self.center_full_height_column_count()).max(0.001);
                    let style = container.style();
                    style.flex_grow = Some(grow);
                    style.flex_shrink = Some(1.0);
                    style.flex_basis = Some(relative(0.).into());
                } else {
                    let size = size_state
                        .and_then(|state| state.size)
                        .unwrap_or_else(|| panel.default_size(window, cx));
                    container = container.w(size);
                    // Allow the fixed-width dock to shrink when there isn't
                    // enough space (e.g. when the sidebar is open). The
                    // stored size is preserved so the dock expands back
                    // when space becomes available.
                    let style = container.style();
                    style.flex_shrink = Some(1.0);
                }
                if let Some(min) = min_size {
                    container = container.min_w(min);
                }
            } else {
                let size = size_state
                    .and_then(|state| state.size)
                    .unwrap_or_else(|| panel.default_size(window, cx));
                container = container.h(size);
            }
        }

        Some(container)
    }

    pub fn agent_panel_position(&self, cx: &App) -> Option<DockPosition> {
        self.all_docks().into_iter().find_map(|dock| {
            let dock = dock.read(cx);
            dock.has_agent_panel(cx).then_some(dock.position())
        })
    }
}
