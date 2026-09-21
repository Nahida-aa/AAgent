use gpui::{Context, Entity};

use super::Workspace;
use crate::dock::Dock;

impl Workspace {
    pub fn add_panel<T: Panel>(
        &mut self,
        panel: Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus_handle = panel.panel_focus_handle(cx);
        cx.on_focus_in(&focus_handle, window, Self::handle_panel_focused)
            .detach();

        let dock_position = panel.position(window, cx);
        let dock = self.dock_at_position(dock_position);
        let any_panel = panel.to_any();
        let persisted_size_state =
            self.persisted_panel_size_state(T::panel_key(), cx)
                .or_else(|| {
                    load_legacy_panel_size(T::panel_key(), dock_position, self, cx).map(|size| {
                        let state = dock::PanelSizeState {
                            size: Some(size),
                            flex: None,
                        };
                        self.persist_panel_size_state(T::panel_key(), state, cx);
                        state
                    })
                });

        dock.update(cx, |dock, cx| {
            let index = dock.add_panel(panel.clone(), self.weak_self.clone(), window, cx);
            if let Some(size_state) = persisted_size_state {
                dock.set_panel_size_state(&panel, size_state, cx);
            }
            index
        });

        cx.emit(Event::PanelAdded(any_panel));
    }

    pub fn remove_panel<T: Panel>(
        &mut self,
        panel: &Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for dock in [&self.left_dock, &self.bottom_dock, &self.right_dock] {
            dock.update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
        }
    }

    /// Transfer focus to the panel of the given type.
    pub fn focus_panel<T: Panel>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<T>> {
        let panel = self.focus_or_unfocus_panel::<T>(window, cx, &mut |_, _, _| true)?;
        panel.to_any().downcast().ok()
    }

    /// Focus the panel of the given type if it isn't already focused. If it is
    /// already focused, then transfer focus back to the workspace center.
    /// When the `close_panel_on_toggle` setting is enabled, also closes the
    /// panel when transferring focus back to the center.
    pub fn toggle_panel_focus<T: Panel>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut did_focus_panel = false;
        self.focus_or_unfocus_panel::<T>(window, cx, &mut |panel, window, cx| {
            did_focus_panel = !panel.panel_focus_handle(cx).contains_focused(window, cx);
            did_focus_panel
        });

        if !did_focus_panel && WorkspaceSettings::get_global(cx).close_panel_on_toggle {
            self.close_panel::<T>(window, cx);
        }

        telemetry::event!(
            "Panel Button Clicked",
            name = T::persistent_name(),
            toggle_state = did_focus_panel
        );

        did_focus_panel
    }

    /// Focus or unfocus the given panel type, depending on the given callback.
    fn focus_or_unfocus_panel<T: Panel>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        should_focus: &mut dyn FnMut(&dyn PanelHandle, &mut Window, &mut Context<Dock>) -> bool,
    ) -> Option<Arc<dyn PanelHandle>> {
        let mut result_panel = None;
        let mut serialize = false;
        for dock in self.all_docks() {
            if let Some(panel_index) = dock.read(cx).panel_index_for_type::<T>() {
                let mut focus_center = false;
                let panel = dock.update(cx, |dock, cx| {
                    dock.activate_panel(panel_index, window, cx);

                    let panel = dock.active_panel().cloned();
                    if let Some(panel) = panel.as_ref() {
                        if should_focus(&**panel, window, cx) {
                            dock.set_open(true, window, cx);
                            panel.activation_focus_handle(cx).focus(window, cx);
                        } else {
                            focus_center = true;
                        }
                    }
                    panel
                });

                if focus_center {
                    self.active_pane
                        .update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx))
                }

                result_panel = panel;
                serialize = true;
                break;
            }
        }

        if serialize {
            self.serialize_workspace(window, cx);
        }

        cx.notify();
        result_panel
    }

    /// Open the panel of the given type
    pub fn open_panel<T: Panel>(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for dock in self.all_docks() {
            if let Some(panel_index) = dock.read(cx).panel_index_for_type::<T>() {
                dock.update(cx, |dock, cx| {
                    dock.activate_panel(panel_index, window, cx);
                    dock.set_open(true, window, cx);
                });
            }
        }
    }

    /// Open the panel of the given type, dismissing any zoomed items that
    /// would obscure it (e.g. a zoomed terminal).
    pub fn reveal_panel<T: Panel>(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dock_position = self.all_docks().iter().find_map(|dock| {
            let dock = dock.read(cx);
            dock.panel_index_for_type::<T>().map(|_| dock.position())
        });
        self.dismiss_zoomed_items_to_reveal(dock_position, window, cx);
        self.open_panel::<T>(window, cx);
    }

    pub fn close_panel<T: Panel>(&self, window: &mut Window, cx: &mut Context<Self>) {
        for dock in self.all_docks().iter() {
            dock.update(cx, |dock, cx| {
                if dock.panel::<T>().is_some() {
                    dock.set_open(false, window, cx)
                }
            })
        }
    }

    pub fn panel<T: Panel>(&self, cx: &App) -> Option<Entity<T>> {
        self.all_docks()
            .iter()
            .find_map(|dock| dock.read(cx).panel::<T>())
    }

    pub fn panel_size_state<T: Panel>(&self, cx: &App) -> Option<dock::PanelSizeState> {
        self.all_docks().into_iter().find_map(|dock| {
            let dock = dock.read(cx);
            let panel = dock.panel::<T>()?;
            dock.stored_panel_size_state(&panel)
        })
    }

    pub fn set_panel_size_state<T: Panel>(
        &mut self,
        size_state: dock::PanelSizeState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self.panel::<T>(cx) else {
            return false;
        };

        let dock = self.dock_at_position(panel.position(window, cx));
        let did_set = dock.update(cx, |dock, cx| {
            dock.set_panel_size_state(&panel, size_state, cx)
        });

        if did_set {
            self.persist_panel_size_state(T::panel_key(), size_state, cx);
        }

        did_set
    }

    pub fn persist_panel_size_state(
        &self,
        panel_key: &str,
        size_state: dock::PanelSizeState,
        cx: &mut App,
    ) {
        let Some(workspace_id) = self
            .database_id()
            .map(|id| i64::from(id).to_string())
            .or(self.session_id())
        else {
            return;
        };

        let kvp = db::kvp::KeyValueStore::global(cx);
        let panel_key = panel_key.to_string();
        cx.background_spawn(async move {
            let scope = kvp.scoped(dock::PANEL_SIZE_STATE_KEY);
            scope
                .write(
                    format!("{workspace_id}:{panel_key}"),
                    serde_json::to_string(&size_state)?,
                )
                .await
        })
        .detach_and_log_err(cx);
    }

    pub fn toggle_dock_panel_flexible_size(
        &self,
        dock: &Entity<Dock>,
        panel: &dyn PanelHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        let position = dock.read(cx).position();
        let current_size = self.dock_size(&dock.read(cx), window, cx);
        let current_flex =
            current_size.and_then(|size| self.dock_flex_for_size(position, size, window, cx));
        dock.update(cx, |dock, cx| {
            dock.toggle_panel_flexible_size(panel, current_size, current_flex, window, cx);
        });
    }

    pub fn activate_panel_for_proto_id(
        &mut self,
        panel_id: PanelId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<dyn PanelHandle>> {
        let mut panel = None;
        for dock in self.all_docks() {
            if let Some(panel_index) = dock.read(cx).panel_index_for_proto_id(panel_id) {
                panel = dock.update(cx, |dock, cx| {
                    dock.activate_panel(panel_index, window, cx);
                    dock.set_open(true, window, cx);
                    dock.active_panel().cloned()
                });
                break;
            }
        }

        if panel.is_some() {
            cx.notify();
            self.serialize_workspace(window, cx);
        }

        panel
    }

    pub fn set_sidebar_focus_handle(&mut self, handle: Option<FocusHandle>) {
        self.sidebar_focus_handle = handle;
    }

    fn handle_panel_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_deferred_saves(window, cx);
        self.update_active_view_for_followers(window, cx);
    }
}
