use super::*;

impl Workspace {
    //
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
    //
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
    //
    pub(super) fn opposite_dock_panel_and_size_state(
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
    //
    pub(super) fn center_full_height_column_count(&self) -> f32 {
        self.center.full_height_column_count().max(1) as f32
    }
    //
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
    //
    fn resize_left_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App) {
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
    //
    fn resize_right_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App) {
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
    //
    fn resize_bottom_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App) {
        let size = new_size.min(self.bounds.bottom() - RESIZE_HANDLE_SIZE - self.bounds.top());
        self.bottom_dock.update(cx, |bottom_dock, cx| {
            bottom_dock.resize_panel_sizes(Some(size), None, window, cx);
        });
    }
}

pub(crate) fn px_with_ui_font_fallback(val: u32, cx: &Context<Workspace>) -> Pixels {
    if val == 0 {
        ThemeSettings::get_global(cx).ui_font_size(cx)
    } else {
        px(val as f32)
    }
}

fn adjust_active_dock_size_by_px(
    px: Pixels,
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(active_dock) = workspace
        .all_docks()
        .into_iter()
        .find(|dock| dock.focus_handle(cx).contains_focused(window, cx))
    else {
        return;
    };
    let dock = active_dock.read(cx);
    let Some(panel_size) = workspace.dock_size(&dock, window, cx) else {
        return;
    };
    workspace.resize_dock(dock.position(), panel_size + px, window, cx);
}

fn adjust_open_docks_size_by_px(
    px: Pixels,
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let docks = workspace
        .all_docks()
        .into_iter()
        .filter_map(|dock_entity| {
            let dock = dock_entity.read(cx);
            if dock.is_open() {
                let dock_pos = dock.position();
                let panel_size = workspace.dock_size(&dock, window, cx)?;
                Some((dock_pos, panel_size + px))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for (position, new_size) in docks {
        workspace.resize_dock(position, new_size, window, cx);
    }
}
