use super::Workspace;
use crate::{dock::Dock, workspace::event::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    /// Multiworkspace uses this to add workspace action handling to itself
    pub fn actions(&self, div: Div, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let active_item_is_read_only = self
            .active_item(cx)
            .is_some_and(|item| !item.capability(cx).editable());

        self.add_workspace_actions_listeners(div, window, cx)
            .on_action(cx.listener(
                |_workspace, action_sequence: &settings::ActionSequence, window, cx| {
                    for action in &action_sequence.0 {
                        window.dispatch_action(action.boxed_clone(), cx);
                    }
                },
            ))
            .on_action(cx.listener(Self::close_inactive_items_and_panes))
            .on_action(cx.listener(Self::close_all_items_and_panes))
            .on_action(cx.listener(Self::close_item_in_all_panes))
            .on_action(cx.listener(Self::save_all))
            .on_action(cx.listener(Self::send_keystrokes))
            .on_action(cx.listener(Self::add_folder_to_project))
            .on_action(cx.listener(Self::follow_next_collaborator))
            .on_action(cx.listener(Self::activate_pane_at_index))
            .on_action(cx.listener(Self::move_item_to_pane_at_index))
            .on_action(cx.listener(Self::move_focused_panel_to_next_position))
            .on_action(cx.listener(Self::reopen_last_picker))
            .on_action(cx.listener(Self::toggle_edit_predictions_all_files))
            .on_action(cx.listener(Self::toggle_theme_mode))
            .on_action(cx.listener(|workspace, _: &Unfollow, window, cx| {
                let pane = workspace.active_pane().clone();
                workspace.unfollow_in_pane(&pane, window, cx);
            }))
            .when(!active_item_is_read_only, |this| {
                this.on_action(cx.listener(|workspace, action: &Save, window, cx| {
                    workspace
                        .save_active_item(
                            action.save_intent.unwrap_or(SaveIntent::Save),
                            window,
                            cx,
                        )
                        .detach_and_prompt_err("Failed to save", window, cx, |_, _, _| None);
                }))
                .on_action(cx.listener(|workspace, _: &FormatAndSave, window, cx| {
                    workspace
                        .save_active_item(SaveIntent::FormatAndSave, window, cx)
                        .detach_and_prompt_err("Failed to save", window, cx, |_, _, _| None);
                }))
                .on_action(cx.listener(
                    |workspace, _: &SaveWithoutFormat, window, cx| {
                        workspace
                            .save_active_item(SaveIntent::SaveWithoutFormat, window, cx)
                            .detach_and_prompt_err("Failed to save", window, cx, |_, _, _| None);
                    },
                ))
            })
            .on_action(cx.listener(|workspace, _: &SaveAs, window, cx| {
                workspace
                    .save_active_item(SaveIntent::SaveAs, window, cx)
                    .detach_and_prompt_err("Failed to save", window, cx, |_, _, _| None);
            }))
            .on_action(
                cx.listener(|workspace, _: &ActivatePreviousPane, window, cx| {
                    workspace.activate_previous_pane(window, cx)
                }),
            )
            .on_action(cx.listener(|workspace, _: &ActivateNextPane, window, cx| {
                workspace.activate_next_pane(window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &ActivateLastPane, window, cx| {
                workspace.activate_last_pane(window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &FocusNextPart, window, cx| {
                workspace.move_part_focus(true, window, cx);
            }))
            .on_action(cx.listener(|workspace, _: &FocusPreviousPart, window, cx| {
                workspace.move_part_focus(false, window, cx);
            }))
            .on_action(
                cx.listener(|workspace, _: &ActivateNextWindow, _window, cx| {
                    workspace.activate_next_window(cx)
                }),
            )
            .on_action(
                cx.listener(|workspace, _: &ActivatePreviousWindow, _window, cx| {
                    workspace.activate_previous_window(cx)
                }),
            )
            .on_action(cx.listener(|workspace, _: &ActivatePaneLeft, window, cx| {
                workspace.activate_pane_in_direction(SplitDirection::Left, window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &ActivatePaneRight, window, cx| {
                workspace.activate_pane_in_direction(SplitDirection::Right, window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &ActivatePaneUp, window, cx| {
                workspace.activate_pane_in_direction(SplitDirection::Up, window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &ActivatePaneDown, window, cx| {
                workspace.activate_pane_in_direction(SplitDirection::Down, window, cx)
            }))
            .on_action(cx.listener(
                |workspace, action: &MoveItemToPaneInDirection, window, cx| {
                    workspace.move_item_to_pane_in_direction(action, window, cx)
                },
            ))
            .on_action(cx.listener(|workspace, _: &SwapPaneLeft, _, cx| {
                workspace.swap_pane_in_direction(SplitDirection::Left, cx)
            }))
            .on_action(cx.listener(|workspace, _: &SwapPaneRight, _, cx| {
                workspace.swap_pane_in_direction(SplitDirection::Right, cx)
            }))
            .on_action(cx.listener(|workspace, _: &SwapPaneUp, _, cx| {
                workspace.swap_pane_in_direction(SplitDirection::Up, cx)
            }))
            .on_action(cx.listener(|workspace, _: &SwapPaneDown, _, cx| {
                workspace.swap_pane_in_direction(SplitDirection::Down, cx)
            }))
            .on_action(cx.listener(|workspace, _: &SwapPaneAdjacent, window, cx| {
                const DIRECTION_PRIORITY: [SplitDirection; 4] = [
                    SplitDirection::Down,
                    SplitDirection::Up,
                    SplitDirection::Right,
                    SplitDirection::Left,
                ];
                for dir in DIRECTION_PRIORITY {
                    if workspace.find_pane_in_direction(dir, cx).is_some() {
                        workspace.swap_pane_in_direction(dir, cx);
                        workspace.activate_pane_in_direction(dir.opposite(), window, cx);
                        break;
                    }
                }
            }))
            .on_action(cx.listener(|workspace, _: &MovePaneLeft, _, cx| {
                workspace.move_pane_to_border(SplitDirection::Left, cx)
            }))
            .on_action(cx.listener(|workspace, _: &MovePaneRight, _, cx| {
                workspace.move_pane_to_border(SplitDirection::Right, cx)
            }))
            .on_action(cx.listener(|workspace, _: &MovePaneUp, _, cx| {
                workspace.move_pane_to_border(SplitDirection::Up, cx)
            }))
            .on_action(cx.listener(|workspace, _: &MovePaneDown, _, cx| {
                workspace.move_pane_to_border(SplitDirection::Down, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleLeftDock, window, cx| {
                this.toggle_dock(DockPosition::Left, window, cx);
            }))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ToggleRightDock, window, cx| {
                    workspace.toggle_dock(DockPosition::Right, window, cx);
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ToggleBottomDock, window, cx| {
                    workspace.toggle_dock(DockPosition::Bottom, window, cx);
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &CloseActiveDock, window, cx| {
                    if !workspace.close_active_dock(window, cx) {
                        cx.propagate();
                    }
                },
            ))
            .on_action(
                cx.listener(|workspace: &mut Workspace, _: &CloseAllDocks, window, cx| {
                    workspace.close_all_docks(window, cx);
                }),
            )
            .on_action(cx.listener(Self::toggle_all_docks))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ClearAllNotifications, _, cx| {
                    workspace.clear_all_notifications(cx);
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ClearNavigationHistory, window, cx| {
                    workspace.clear_navigation_history(window, cx);
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &SuppressNotification, _, cx| {
                    if let Some((notification_id, _)) = workspace.notifications.pop() {
                        workspace.suppress_notification(&notification_id, cx);
                    }
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ToggleWorktreeSecurity, window, cx| {
                    workspace.show_worktree_trust_security_modal(true, window, cx);
                },
            ))
            .on_action(
                cx.listener(|_: &mut Workspace, _: &ClearTrustedWorktrees, _, cx| {
                    if let Some(trusted_worktrees) = TrustedWorktrees::try_get_global(cx) {
                        trusted_worktrees.update(cx, |trusted_worktrees, _| {
                            trusted_worktrees.clear_trusted_paths()
                        });
                        let db = WorkspaceDb::global(cx);
                        cx.spawn(async move |_, cx| {
                            if db.clear_trusted_worktrees().await.log_err().is_some() {
                                cx.update(|cx| reload(cx));
                            }
                        })
                        .detach();
                    }
                }),
            )
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ReopenClosedItem, window, cx| {
                    workspace.reopen_closed_item(window, cx).detach();
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ResetActiveDockSize, window, cx| {
                    if let Some(dock) = workspace.active_dock(window, cx).cloned() {
                        dock.update(cx, |dock, cx| {
                            dock.reset_panel_sizes(window, cx);
                        });
                    }
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ResetOpenDocksSize, window, cx| {
                    for dock in workspace.all_docks() {
                        if dock.read(cx).visible_panel().is_some() {
                            dock.update(cx, |dock, cx| {
                                dock.reset_panel_sizes(window, cx);
                            });
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, _: &ResetPaneSizes, _window, cx| {
                    workspace.reset_pane_sizes(cx);
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, act: &IncreaseActiveDockSize, window, cx| {
                    adjust_active_dock_size_by_px(
                        px_with_ui_font_fallback(act.px, cx),
                        workspace,
                        window,
                        cx,
                    );
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, act: &DecreaseActiveDockSize, window, cx| {
                    adjust_active_dock_size_by_px(
                        px_with_ui_font_fallback(act.px, cx) * -1.,
                        workspace,
                        window,
                        cx,
                    );
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, act: &IncreaseOpenDocksSize, window, cx| {
                    adjust_open_docks_size_by_px(
                        px_with_ui_font_fallback(act.px, cx),
                        workspace,
                        window,
                        cx,
                    );
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, act: &DecreaseOpenDocksSize, window, cx| {
                    adjust_open_docks_size_by_px(
                        px_with_ui_font_fallback(act.px, cx) * -1.,
                        workspace,
                        window,
                        cx,
                    );
                },
            ))
            .on_action(cx.listener(Workspace::toggle_centered_layout))
            .on_action(cx.listener(Workspace::toggle_editor_zoom))
            .on_action(cx.listener(
                |workspace: &mut Workspace, action: &pane::ActivateNextItem, window, cx| {
                    if let Some(active_dock) = workspace.active_dock(window, cx) {
                        let dock = active_dock.read(cx);
                        if let Some(active_panel) = dock.active_panel() {
                            if active_panel.pane(cx).is_none() {
                                let mut recent_pane: Option<Entity<Pane>> = None;
                                let mut recent_timestamp = 0;
                                for pane_handle in workspace.panes() {
                                    let pane = pane_handle.read(cx);
                                    for entry in pane.activation_history() {
                                        if entry.timestamp > recent_timestamp {
                                            recent_timestamp = entry.timestamp;
                                            recent_pane = Some(pane_handle.clone());
                                        }
                                    }
                                }

                                if let Some(pane) = recent_pane {
                                    let wrap_around = action.wrap_around;
                                    pane.update(cx, |pane, cx| {
                                        let current_index = pane.active_item_index();
                                        let items_len = pane.items_len();
                                        if items_len > 0 {
                                            let next_index = if current_index + 1 < items_len {
                                                current_index + 1
                                            } else if wrap_around {
                                                0
                                            } else {
                                                return;
                                            };
                                            pane.activate_item(
                                                next_index, false, false, window, cx,
                                            );
                                        }
                                    });
                                    return;
                                }
                            }
                        }
                    }
                    cx.propagate();
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, action: &pane::ActivatePreviousItem, window, cx| {
                    if let Some(active_dock) = workspace.active_dock(window, cx) {
                        let dock = active_dock.read(cx);
                        if let Some(active_panel) = dock.active_panel() {
                            if active_panel.pane(cx).is_none() {
                                let mut recent_pane: Option<Entity<Pane>> = None;
                                let mut recent_timestamp = 0;
                                for pane_handle in workspace.panes() {
                                    let pane = pane_handle.read(cx);
                                    for entry in pane.activation_history() {
                                        if entry.timestamp > recent_timestamp {
                                            recent_timestamp = entry.timestamp;
                                            recent_pane = Some(pane_handle.clone());
                                        }
                                    }
                                }

                                if let Some(pane) = recent_pane {
                                    let wrap_around = action.wrap_around;
                                    pane.update(cx, |pane, cx| {
                                        let current_index = pane.active_item_index();
                                        let items_len = pane.items_len();
                                        if items_len > 0 {
                                            let prev_index = if current_index > 0 {
                                                current_index - 1
                                            } else if wrap_around {
                                                items_len.saturating_sub(1)
                                            } else {
                                                return;
                                            };
                                            pane.activate_item(
                                                prev_index, false, false, window, cx,
                                            );
                                        }
                                    });
                                    return;
                                }
                            }
                        }
                    }
                    cx.propagate();
                },
            ))
            .on_action(cx.listener(
                |workspace: &mut Workspace, action: &pane::CloseActiveItem, window, cx| {
                    if let Some(active_dock) = workspace.active_dock(window, cx) {
                        let dock = active_dock.read(cx);
                        if let Some(active_panel) = dock.active_panel() {
                            if active_panel.pane(cx).is_none() {
                                let active_pane = workspace.active_pane().clone();
                                active_pane.update(cx, |pane, cx| {
                                    pane.close_active_item(action, window, cx)
                                        .detach_and_log_err(cx);
                                });
                                return;
                            }
                        }
                    }
                    cx.propagate();
                },
            ))
            .on_action(
                cx.listener(|workspace, _: &ToggleReadOnlyFile, window, cx| {
                    let pane = workspace.active_pane().clone();
                    if let Some(item) = pane.read(cx).active_item() {
                        item.toggle_read_only(window, cx);
                    }
                }),
            )
            .on_action(cx.listener(|workspace, _: &FocusCenterPane, window, cx| {
                workspace.focus_center_pane(window, cx);
            }))
            .on_action(cx.listener(Workspace::clear_bookmarks))
            .on_action(cx.listener(Workspace::cancel))
    }

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
