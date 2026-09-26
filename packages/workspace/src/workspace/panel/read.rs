
use super::*;
impl Workspace {

    // panel_size_state<T>
    pub fn panel_size_state<T: Panel>(&self, cx: &App) -> Option<dock::PanelSizeState> {
        self.all_docks().into_iter().find_map(|dock| {
            let dock = dock.read(cx);
            let panel = dock.panel::<T>()?;
            dock.stored_panel_size_state(&panel)
        })
    }
    // persisted_panel_size_state
    pub fn persisted_panel_size_state(
        &self,
        panel_key: &'static str,
        cx: &App,
    ) -> Option<dock::PanelSizeState> {
        dock::Dock::load_persisted_size_state(self, panel_key, cx)
    }
    //  agent_panel_position
    pub fn agent_panel_position(&self, cx: &App) -> Option<DockPosition> {
        self.all_docks().into_iter().find_map(|dock| {
            let dock = dock.read(cx);
            dock.has_agent_panel(cx).then_some(dock.position())
        })
    }
    // panel<T>
    pub fn panel<T: Panel>(&self, cx: &App) -> Option<Entity<T>> {
        self.all_docks()
            .iter()
            .find_map(|dock| dock.read(cx).panel::<T>())
    }
}
