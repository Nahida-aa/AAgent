use super::*;
impl Workspace {

    // pub fn fallback_focus_handle      // 焦点回退，主语是 pane
    // If a dock panel is zoomed, focus it instead of the center pane.
    // Otherwise, focusing the center pane triggers dismiss_zoomed_items_to_reveal
    // which closes the zoomed dock.
    pub fn fallback_focus_handle(&self, window: &Window, cx: &App) -> FocusHandle {
        self.all_docks()
            .into_iter()
            .find_map(|dock| {
                let dock = dock.read(cx);
                if !dock.is_open() {
                    return None;
                }
                let panel = dock.active_panel()?;
                if panel.is_zoomed(window, cx) {
                    Some(panel.activation_focus_handle(cx))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.active_pane.read(cx).focus_handle(cx))
    }
    // pub fn panes_mut
    pub fn panes_mut(&mut self) -> &mut [Entity<Pane>] {
        &mut self.panes
    }
    // pub fn panes
    pub fn panes(&self) -> &[Entity<Pane>] {
        &self.panes
    }
    // pub fn active_pane
    pub fn active_pane(&self) -> &Entity<Pane> {
        &self.active_pane
    }
    // pub fn focused_pane
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

    // pub fn pane_for
    pub fn pane_for(&self, handle: &dyn ItemHandle) -> Option<Entity<Pane>> {
        self.pane_for_item_id(handle.item_id())
    }
    // pub fn pane_for_item_id
    pub fn pane_for_item_id(&self, item_id: EntityId) -> Option<Entity<Pane>> {
        let weak_pane = self.panes_by_item.get(&item_id)?;
        weak_pane.upgrade()
    }
    // pub fn pane_for_entity_id
    pub fn pane_for_entity_id(&self, entity_id: EntityId) -> Option<Entity<Pane>> {
        self.panes
            .iter()
            .find(|pane| pane.entity_id() == entity_id)
            .cloned()
    }
    // pub fn bounding_box_for_pane
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
