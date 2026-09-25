impl Workspace {
    // pub fn close_global
    pub fn close_global(cx: &mut App) {
        cx.defer(|cx| {
            cx.windows().iter().find(|window| {
                window
                    .update(cx, |_, window, _| {
                        if window.is_window_active() {
                            //This can only get called when the window's project connection has been lost
                            //so we don't need to prompt the user for anything and instead just close the window
                            window.remove_window();
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
            });
        });
    }
    // ├── on_window_activation_changed
    pub fn on_window_activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_window_active() {
            if let Some(database_id) = self.database_id {
                let db = WorkspaceDb::global(cx);
                cx.background_spawn(async move { db.update_timestamp(database_id).await })
                    .detach();
            }
        } else {
            // When window is deactivated, flush any deferred saves since focus has left the window
            self.flush_deferred_saves(window, cx);
            for pane in &self.panes {
                pane.update(cx, |pane, cx| {
                    if let Some(item) = pane.active_item() {
                        item.workspace_deactivated(window, cx);
                    }
                    for item in pane.items() {
                        if matches!(
                            item.workspace_settings(cx).autosave,
                            AutosaveSetting::OnWindowChange | AutosaveSetting::OnFocusChange
                        ) {
                            Pane::autosave_item(item.as_ref(), self.project.clone(), window, cx)
                                .detach_and_log_err(cx);
                        }
                    }
                });
            }
        }
    }
    // ├── activate_next_window
    pub fn activate_next_window(&mut self, cx: &mut Context<Self>) {
        let Some(current_window_id) = cx.active_window().map(|a| a.window_id()) else {
            return;
        };
        let windows = cx.windows();
        let next_window =
            SystemWindowTabController::get_next_tab_group_window(cx, current_window_id).or_else(
                || {
                    windows
                        .iter()
                        .cycle()
                        .skip_while(|window| window.window_id() != current_window_id)
                        .nth(1)
                },
            );

        if let Some(window) = next_window {
            window
                .update(cx, |_, window, _| window.activate_window())
                .ok();
        }
    }
    // ├── activate_previous_window
    pub fn activate_previous_window(&mut self, cx: &mut Context<Self>) {
        let Some(current_window_id) = cx.active_window().map(|a| a.window_id()) else {
            return;
        };
        let windows = cx.windows();
        let prev_window =
            SystemWindowTabController::get_prev_tab_group_window(cx, current_window_id).or_else(
                || {
                    windows
                        .iter()
                        .rev()
                        .cycle()
                        .skip_while(|window| window.window_id() != current_window_id)
                        .nth(1)
                },
            );

        if let Some(window) = prev_window {
            window
                .update(cx, |_, window, _| window.activate_window())
                .ok();
        }
    }

}

// └── activate_any_workspace_window
pub fn activate_any_workspace_window(cx: &mut AsyncApp) -> Option<WindowHandle<MultiWorkspace>> {
    cx.update(|cx| {
        if let Some(workspace_window) = cx
            .active_window()
            .and_then(|window| window.downcast::<MultiWorkspace>())
        {
            return Some(workspace_window);
        }

        for window in cx.windows() {
            if let Some(workspace_window) = window.downcast::<MultiWorkspace>() {
                workspace_window
                    .update(cx, |_, window, _| window.activate_window())
                    .ok();
                return Some(workspace_window);
            }
        }
        None
    })
}
