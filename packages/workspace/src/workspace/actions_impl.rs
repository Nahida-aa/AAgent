use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    pub fn register_action<A: Action>(
        &mut self,
        callback: impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) + 'static,
    ) -> &mut Self {
        let callback = Arc::new(callback);

        self.workspace_actions.push(Box::new(move |div, _, _, cx| {
            let callback = callback.clone();
            div.on_action(cx.listener(move |workspace, event, window, cx| {
                (callback)(workspace, event, window, cx)
            }))
        }));
        self
    }
    pub fn register_action_renderer(
        &mut self,
        callback: impl Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div + 'static,
    ) -> &mut Self {
        self.workspace_actions.push(Box::new(callback));
        self
    }

    fn add_workspace_actions_listeners(
        &self,
        mut div: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        for action in self.workspace_actions.iter() {
            div = (action)(div, self, window, cx)
        }
        div
    }

    fn send_keystrokes(
        &mut self,
        action: &SendKeystrokes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystrokes: Vec<Keystroke> = action
            .0
            .split(' ')
            .flat_map(|k| Keystroke::parse(k).log_err())
            .map(|k| {
                cx.keyboard_mapper()
                    .map_key_equivalent(k, false)
                    .inner()
                    .clone()
            })
            .collect();
        let _ = self.send_keystrokes_impl(keystrokes, window, cx);
    }

    pub fn send_keystrokes_impl(
        &mut self,
        keystrokes: Vec<Keystroke>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Shared<Task<()>> {
        let mut state = self.dispatching_keystrokes.borrow_mut();
        if !state.dispatched.insert(keystrokes.clone()) {
            cx.propagate();
            return state.task.clone().unwrap();
        }

        state.queue.extend(keystrokes);

        let keystrokes = self.dispatching_keystrokes.clone();
        if state.task.is_none() {
            state.task = Some(
                window
                    .spawn(cx, async move |cx| {
                        // limit to 100 keystrokes to avoid infinite recursion.
                        for _ in 0..100 {
                            let keystroke = {
                                let mut state = keystrokes.borrow_mut();
                                let Some(keystroke) = state.queue.pop_front() else {
                                    state.dispatched.clear();
                                    state.task.take();
                                    return;
                                };
                                keystroke
                            };
                            let focus_changed = cx
                                .update(|window, cx| {
                                    let focused = window.focused(cx);
                                    window.dispatch_keystroke(keystroke.clone(), cx);
                                    if window.focused(cx) != focused {
                                        // dispatch_keystroke may cause the focus to change.
                                        // draw's side effect is to schedule the FocusChanged events in the current flush effect cycle
                                        // And we need that to happen before the next keystroke to keep vim mode happy...
                                        // (Note that the tests always do this implicitly, so you must manually test with something like:
                                        //   "bindings": { "g z": ["workspace::SendKeystrokes", ": j <enter> u"]}
                                        // )
                                        window.draw(cx).clear(cx);
                                        return true;
                                    }
                                    false
                                })
                                .unwrap_or(false);

                            if focus_changed {
                                futures_lite::future::yield_now().await;
                            }
                        }

                        *keystrokes.borrow_mut() = Default::default();
                        log::error!("over 100 keystrokes passed to send_keystrokes");
                    })
                    .shared(),
            );
        }
        state.task.clone().unwrap()
    }

    pub fn cancel(&mut self, _: &menu::Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if cx.stop_active_drag(window) {
        } else if let Some((notification_id, _)) = self.notifications.pop() {
            dismiss_app_notification(&notification_id, cx);
        } else {
            cx.propagate();
        }
    }

    pub fn toggle_centered_layout(
        &mut self,
        _: &ToggleCenteredLayout,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.centered_layout = !self.centered_layout;
        if let Some(database_id) = self.database_id() {
            let db = WorkspaceDb::global(cx);
            let centered_layout = self.centered_layout;
            cx.background_spawn(async move {
                db.set_centered_layout(database_id, centered_layout).await
            })
            .detach_and_log_err(cx);
        }
        cx.notify();
    }

    pub fn toggle_editor_zoom(
        &mut self,
        _: &ToggleEditorZoom,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.zoomed.is_some() {
            self.active_pane.update(cx, |pane, cx| {
                pane.set_zoomed(false, cx);
            });
            self.zoomed = None;
            self.zoomed_position = None;
            cx.emit(Event::ZoomChanged);
        }

        if let Some(maximized) = self.maximized_pane.take() {
            if maximized.upgrade().as_ref() == Some(&self.active_pane) {
                cx.notify();
                return;
            }
        }

        self.maximized_pane = Some(self.active_pane.downgrade());
        window.focus(&self.active_pane.focus_handle(cx), cx);
        cx.notify();
    }

    pub fn is_pane_maximized(&self) -> bool { self.maximized_pane.is_some() }

    pub fn clear_bookmarks(&mut self, _: &ClearBookmarks, _: &mut Window, cx: &mut Context<Self>) {
        self.project()
            .read(cx)
            .bookmark_store()
            .update(cx, |bookmark_store, cx| {
                bookmark_store.clear_bookmarks(cx);
            });
    }

    fn toggle_edit_predictions_all_files(
        &mut self,
        _: &ToggleEditPrediction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fs = self.project().read(cx).fs().clone();
        let show_edit_predictions = all_language_settings(None, cx).show_edit_predictions(None, cx);
        update_settings_file(fs, cx, move |file, _| {
            file.project.all_languages.defaults.show_edit_predictions = Some(!show_edit_predictions)
        });
    }

    fn toggle_theme_mode(&mut self, _: &ToggleMode, _window: &mut Window, cx: &mut Context<Self>) {
        let current_mode = ThemeSettings::get_global(cx).theme.mode();
        let next_mode = match current_mode {
            Some(theme_settings::ThemeAppearanceMode::Light) => {
                theme_settings::ThemeAppearanceMode::Dark
            }
            Some(theme_settings::ThemeAppearanceMode::Dark) => {
                theme_settings::ThemeAppearanceMode::Light
            }
            Some(theme_settings::ThemeAppearanceMode::System) | None => {
                match cx.theme().appearance() {
                    theme::Appearance::Light => theme_settings::ThemeAppearanceMode::Dark,
                    theme::Appearance::Dark => theme_settings::ThemeAppearanceMode::Light,
                }
            }
        };

        let fs = self.project().read(cx).fs().clone();
        settings::update_settings_file(fs, cx, move |settings, _cx| {
            theme_settings::set_mode(settings, next_mode);
        });
    }

    /// Toggles all docks between open and closed states.
    ///
    /// If any docks are open, closes all and remembers their positions. If all
    /// docks are closed, restores the last remembered dock configuration.
    fn toggle_all_docks(
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

    pub fn move_focused_panel_to_next_position(
        &mut self,
        _: &MoveFocusedPanelToNextPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let docks = self.all_docks();
        let active_dock = docks
            .into_iter()
            .find(|dock| dock.focus_handle(cx).contains_focused(window, cx));

        if let Some(dock) = active_dock {
            dock.update(cx, |dock, cx| {
                let active_panel = dock
                    .active_panel()
                    .filter(|panel| panel.panel_focus_handle(cx).contains_focused(window, cx));

                if let Some(panel) = active_panel {
                    panel.move_to_next_position(window, cx);
                }
            })
        }
    }

    pub(super) fn adjust_padding(padding: Option<f32>) -> f32 {
        padding
            .unwrap_or(CenteredPaddingSettings::default().0)
            .clamp(
                CenteredPaddingSettings::MIN_PADDING,
                CenteredPaddingSettings::MAX_PADDING,
            )
    }
}
