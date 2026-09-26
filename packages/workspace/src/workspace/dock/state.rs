use super::*;

impl Workspace {
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
    //
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
    // dock 的恢复流程
    pub fn finish_dock_restoration(&self, cx: &mut App) {
        for dock in [&self.left_dock, &self.bottom_dock, &self.right_dock] {
            dock.update(cx, |dock, _| {
                dock.finish_restoration();
            });
        }
    }
}
