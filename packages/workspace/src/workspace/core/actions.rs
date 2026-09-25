use gpui::actions;

/// Opens a file or directory.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct Open {
    /// When true, opens in a new window. When false, adds to the current
    /// window as a new workspace (multi-workspace). When omitted, uses
    /// `default_open_behavior`.
    #[serde(default)]
    pub create_new_window: Option<bool>,
}
impl Open {
    pub const DEFAULT: Self = Self {
        create_new_window: None,
    };
}

impl Default for Open {
    fn default() -> Self { Self::DEFAULT }
}

/// Activates a specific pane by its index.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct ActivatePane(pub usize);

/// Moves an item to a specific pane by index.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct MoveItemToPane {
    #[serde(default = "default_1")]
    pub destination: usize,
    #[serde(default = "default_true")]
    pub focus: bool,
    #[serde(default)]
    pub clone: bool,
}

fn default_1() -> usize { 1 }
/// Moves an item to a pane in the specified direction.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct MoveItemToPaneInDirection {
    #[serde(default = "default_right")]
    pub direction: SplitDirection,
    #[serde(default = "default_true")]
    pub focus: bool,
    #[serde(default)]
    pub clone: bool,
}

/// Creates a new file in a split of the desired direction.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewFileSplit(pub SplitDirection);
fn default_right() -> SplitDirection { SplitDirection::Right }

/// Saves all open files in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct SaveAll {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Saves the current file with the specified options.
#[derive(Clone, PartialEq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct Save {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Moves Focus to the central panes in the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = workspace)]
pub struct FocusCenterPane;

///  Closes all items and panes in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseAllItemsAndPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Closes all inactive tabs and panes in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseInactiveTabsAndPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Closes the active item across all panes.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseItemInAllPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
    #[serde(default)]
    pub close_pinned: bool,
}

/// Sends a sequence of keystrokes to the active element.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct SendKeystrokes(pub String);

actions!(
    workspace,
    [
        /// Activates the next pane in the workspace.
        ActivateNextPane,
        /// Activates the previous pane in the workspace.
        ActivatePreviousPane,
        /// Activates the last pane in the workspace.
        ActivateLastPane,
        /// Moves focus to the next major region of the window (editor, open
        /// panels, status bar), cycling and wrapping around. Intended as a
        /// discoverable, screen-reader-friendly way to navigate the window.
        FocusNextPart,
        /// Moves focus to the previous major region of the window. See
        /// [`FocusNextPart`].
        FocusPreviousPart,
        /// Switches to the next window.
        ActivateNextWindow,
        /// Switches to the previous window.
        ActivatePreviousWindow,
        /// Adds a folder to the current project.
        AddFolderToProject,
        /// Clears all bookmarks in the project.
        ClearBookmarks,
        /// Clears all notifications.
        ClearAllNotifications,
        /// Clears all navigation history, including forward/backward navigation, recently opened files, and recently closed tabs. **This action is irreversible**.
        ClearNavigationHistory,
        /// Closes the active dock.
        CloseActiveDock,
        /// Closes all docks.
        CloseAllDocks,
        /// Toggles all docks.
        ToggleAllDocks,
        /// Closes the current window.
        CloseWindow,
        /// Closes the current project.
        CloseProject,
        /// Opens the feedback dialog.
        Feedback,
        /// Follows the next collaborator in the session.
        FollowNextCollaborator,
        /// Moves the focused panel to the next position.
        MoveFocusedPanelToNextPosition,
        /// Creates a new file.
        NewFile,
        /// Creates a new file in a vertical split.
        NewFileSplitVertical,
        /// Creates a new file in a horizontal split.
        NewFileSplitHorizontal,
        /// Opens a new search.
        NewSearch,
        /// Opens a new window.
        NewWindow,
        /// Opens multiple files.
        OpenFiles,
        /// Opens the current location in terminal.
        OpenInTerminal,
        /// Opens the component preview.
        OpenComponentPreview,
        /// Reloads the active item.
        ReloadActiveItem,
        /// Reopens the most recently dismissed picker in the current window.
        ReopenLastPicker,
        /// Resets the active dock to its default size.
        ResetActiveDockSize,
        /// Resets all open docks to their default sizes.
        ResetOpenDocksSize,
        /// Resets all panes in the center group to equal sizes, preserving the split layout.
        ResetPaneSizes,
        /// Reloads the application
        Reload,
        /// Formats and saves the current file, regardless of the format_on_save setting.
        FormatAndSave,
        /// Saves the current file with a new name.
        SaveAs,
        /// Saves without formatting.
        SaveWithoutFormat,
        /// Shuts down all debug adapters.
        ShutdownDebugAdapters,
        /// Suppresses the current notification.
        SuppressNotification,
        /// Toggles the bottom dock.
        ToggleBottomDock,
        /// Toggles centered layout mode.
        ToggleCenteredLayout,
        /// Toggles edit prediction feature globally for all files.
        ToggleEditPrediction,
        /// Toggles the left dock.
        ToggleLeftDock,
        /// Toggles the right dock.
        ToggleRightDock,
        /// Toggles zoom on the active pane.
        ToggleZoom,
        /// Toggles maximizing the active editor pane within the center area,
        /// hiding other split panes but leaving docks/panels unaffected.
        ToggleEditorZoom,
        /// Toggles read-only mode for the active item (if supported by that item).
        ToggleReadOnlyFile,
        /// Zooms in on the active pane.
        ZoomIn,
        /// Zooms out of the active pane.
        ZoomOut,
        /// If any worktrees are in restricted mode, shows a modal with possible actions.
        /// If the modal is shown already, closes it without trusting any worktree.
        ToggleWorktreeSecurity,
        /// Clears all trusted worktrees, placing them in restricted mode on next open.
        /// Requires restart to take effect on already opened projects.
        ClearTrustedWorktrees,
        /// Stops following a collaborator.
        Unfollow,
        /// Restores the banner.
        RestoreBanner,
        /// Toggles expansion of the selected item.
        ToggleExpandItem,
    ]
);

/// Opens a new terminal with the specified working directory.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct OpenTerminal {
    pub working_directory: PathBuf,
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}

impl Workspace {
    //
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
    //
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
    //
    pub fn register_action_renderer(
        &mut self,
        callback: impl Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div + 'static,
    ) -> &mut Self {
        self.workspace_actions.push(Box::new(callback));
        self
    }
    //
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
}
