// ├── DraggedDock（含 DockPosition 字段）
//  └── impl Render for DraggedDock
impl Workspace {
    fn render_dock(
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
}

#[derive(Clone)]
pub struct DraggedDock(pub DockPosition);

impl Render for DraggedDock {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}
