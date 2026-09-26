use super::*;

impl Workspace {
    // left_dock
    pub fn left_dock(&self) -> &Entity<Dock> { &self.left_dock }
    // bottom_dock
    pub fn bottom_dock(&self) -> &Entity<Dock> { &self.bottom_dock }
    // right_dock
    pub fn right_dock(&self) -> &Entity<Dock> { &self.right_dock }
    // all_docks
    pub fn all_docks(&self) -> [&Entity<Dock>; 3] {
        [&self.left_dock, &self.bottom_dock, &self.right_dock]
    }
    // focused_dock_position
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
    // dock_at_position
    pub fn dock_at_position(&self, position: DockPosition) -> &Entity<Dock> {
        match position {
            DockPosition::Left => &self.left_dock,
            DockPosition::Bottom => &self.bottom_dock,
            DockPosition::Right => &self.right_dock,
        }
    }
    // is_dock_at_position_open
    pub fn is_dock_at_position_open(&self, position: DockPosition, cx: &mut Context<Self>) -> bool {
        self.dock_at_position(position).read(cx).is_open()
    }
    // active_dock   返回当前拥有焦点的 dock, private
    pub(crate) fn active_dock(&self, window: &Window, cx: &Context<Self>) -> Option<&Entity<Dock>> {
        self.all_docks().into_iter().find(|&dock| {
            dock.read(cx).is_open() && dock.focus_handle(cx).contains_focused(window, cx)
        })
    }

    pub(crate) fn get_open_dock_positions(&self, cx: &Context<Self>) -> Vec<DockPosition> {
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
}
