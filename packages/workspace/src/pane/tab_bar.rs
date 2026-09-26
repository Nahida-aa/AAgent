use super::*;

use gpui::{App, Context, FocusOutEvent, Window};
use project::Project;
use settings::Settings;
use theme_settings::ThemeSettings;

use super::Pane;
use super::history::{NavigationMode, TagNavigationMode};
use crate::item::{ItemSettings, PreviewTabsSettings};
use crate::workspace_settings::{FocusFollowsMouse, TabBarSettings, WorkspaceSettings};

impl Pane {
    pub(super) fn tab_icon_element(
        &self,
        item: &dyn ItemHandle,
        is_active: bool,
        window: &Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let icon = item
            .tab_icon(window, cx)?
            .size(IconSize::Small)
            .color(Color::Muted);

        let item_diagnostic = item
            .project_path(cx)
            .and_then(|project_path| self.diagnostics.get(&project_path));

        let Some(diagnostic) = item_diagnostic else {
            return Some(icon.into_any_element());
        };

        let knockout_item_color = if is_active {
            cx.theme().colors().tab_active_background
        } else {
            cx.theme().colors().tab_bar_background
        };

        let (icon_decoration, icon_color) = if matches!(diagnostic, &DiagnosticSeverity::ERROR) {
            (IconDecorationKind::X, Color::Error)
        } else {
            (IconDecorationKind::Triangle, Color::Warning)
        };

        Some(
            DecoratedIcon::new(
                icon,
                Some(
                    IconDecoration::new(icon_decoration, knockout_item_color, cx)
                        .color(icon_color.color(cx))
                        .position(Point {
                            x: px(-2.),
                            y: px(-2.),
                        }),
                ),
            )
            .into_any_element(),
        )
    }

    pub(super) fn render_tab(
        &self,
        ix: usize,
        item: &dyn ItemHandle,
        detail: usize,
        focus_handle: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> impl IntoElement + use<> {
        let is_active = ix == self.active_item_index;
        let is_preview = self
            .preview_item_id
            .map(|id| id == item.item_id())
            .unwrap_or(false);

        let label = item.tab_content(
            TabContentParams {
                detail: Some(detail),
                selected: is_active,
                preview: is_preview,
                deemphasized: !self.has_focus(window, cx),
                max_title_len: None,
                truncate_title_middle: false,
            },
            window,
            cx,
        );

        let icon = self.tab_icon_element(item, is_active, window, cx);

        let settings = ItemSettings::get_global(cx);
        let close_side = &settings.close_position;
        let show_close_button = &settings.show_close_button;
        let indicator = render_item_indicator(item.boxed_clone(), cx);
        let tab_tooltip_content = item.tab_tooltip_content(cx);
        let item_id = item.item_id();
        let is_first_item = ix == 0;
        let is_last_item = ix == self.items.len() - 1;
        let is_pinned = self.is_tab_pinned(ix);
        let position_relative_to_active_item = ix.cmp(&self.active_item_index);

        let read_only_toggle = |toggleable: bool| {
            IconButton::new("toggle_read_only", IconName::FileLock)
                .size(ButtonSize::None)
                .shape(IconButtonShape::Square)
                .icon_color(Color::Muted)
                .icon_size(IconSize::Small)
                .disabled(!toggleable)
                .tooltip(move |_, cx| {
                    if toggleable {
                        Tooltip::with_meta(
                            "Unlock Tab",
                            None,
                            "This will make this tab editable",
                            cx,
                        )
                    } else {
                        Tooltip::with_meta("Locked Tab", None, "This tab is read-only", cx)
                    }
                })
                .on_click(cx.listener(move |pane, _, window, cx| {
                    if let Some(item) = pane.item_for_index(ix) {
                        item.toggle_read_only(window, cx);
                    }
                }))
        };

        let has_file_icon = icon.is_some();

        let capability = item.capability(cx);
        let tab = Tab::new(ix)
            .position(if is_first_item {
                TabPosition::First
            } else if is_last_item {
                TabPosition::Last
            } else {
                TabPosition::Middle(position_relative_to_active_item)
            })
            .close_side(match close_side {
                ClosePosition::Left => ui::TabCloseSide::Start,
                ClosePosition::Right => ui::TabCloseSide::End,
            })
            .toggle_state(is_active)
            .on_click(cx.listener({
                let item_handle = item.boxed_clone();
                move |pane: &mut Self, event: &ClickEvent, window, cx| {
                    if event.click_count() > 1 {
                        pane.unpreview_item_if_preview(item_id);
                        let extra_actions = item_handle.tab_extra_context_menu_actions(window, cx);
                        if let Some((_, action)) = extra_actions
                            .into_iter()
                            .find(|(label, _)| label.as_ref() == "Rename")
                        {
                            // Dispatch action directly through the focus handle to avoid
                            // relay_action's intermediate focus step which can interfere
                            // with inline editors.
                            let focus_handle = item_handle.item_focus_handle(cx);
                            focus_handle.dispatch_action(&*action, window, cx);
                            return;
                        }
                    }
                    pane.activate_item(ix, true, true, window, cx)
                }
            }))
            .on_aux_click(
                cx.listener(move |pane: &mut Self, event: &ClickEvent, window, cx| {
                    if !event.is_middle_click() || is_pinned {
                        return;
                    }

                    pane.close_item_by_id(item_id, SaveIntent::Close, window, cx)
                        .detach_and_log_err(cx);
                    cx.stop_propagation();
                }),
            )
            .on_drag(
                DraggedTab {
                    item: item.boxed_clone(),
                    pane: cx.entity(),
                    detail,
                    is_active,
                    ix,
                },
                |tab, _, _, cx| cx.new(|_| tab.clone()),
            )
            .drag_over::<DraggedTab>(move |tab, dragged_tab: &DraggedTab, _, cx| {
                let mut styled_tab = tab
                    .bg(cx.theme().colors().drop_target_background)
                    .border_color(cx.theme().colors().drop_target_border)
                    .border_0();

                if ix < dragged_tab.ix {
                    styled_tab = styled_tab.border_l_2();
                } else if ix > dragged_tab.ix {
                    styled_tab = styled_tab.border_r_2();
                }

                styled_tab
            })
            .drag_over::<DraggedSelection>(|tab, _, _, cx| {
                tab.bg(cx.theme().colors().drop_target_background)
            })
            .when_some(self.can_drop_predicate.clone(), |this, p| {
                this.can_drop(move |a, window, cx| p(a, window, cx))
            })
            .on_drop(
                cx.listener(move |this, dragged_tab: &DraggedTab, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_tab_drop(dragged_tab, ix, false, window, cx)
                }),
            )
            .on_drop(
                cx.listener(move |this, selection: &DraggedSelection, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_dragged_selection_drop(selection, Some(ix), window, cx)
                }),
            )
            .on_drop(cx.listener(move |this, paths, window, cx| {
                this.drag_split_direction = None;
                this.handle_external_paths_drop(paths, window, cx)
            }))
            .start_slot::<Indicator>(indicator)
            .map(|this| {
                let end_slot_action: &'static dyn Action;
                let end_slot_tooltip_text: &'static str;
                let end_slot = if is_pinned {
                    end_slot_action = &TogglePinTab;
                    end_slot_tooltip_text = "Unpin Tab";
                    IconButton::new("unpin tab", IconName::Pin)
                        .shape(IconButtonShape::Square)
                        .icon_color(Color::Muted)
                        .size(ButtonSize::None)
                        .icon_size(IconSize::Small)
                        .on_click(cx.listener(move |pane, _, window, cx| {
                            pane.unpin_tab_at(ix, window, cx);
                        }))
                } else {
                    end_slot_action = &CloseActiveItem {
                        save_intent: None,
                        close_pinned: false,
                    };
                    end_slot_tooltip_text = "Close Tab";
                    match show_close_button {
                        ShowCloseButton::Always => IconButton::new("close tab", IconName::Close),
                        ShowCloseButton::Hover => {
                            IconButton::new("close tab", IconName::Close).visible_on_hover("")
                        }
                        ShowCloseButton::Hidden => return this,
                    }
                    .shape(IconButtonShape::Square)
                    .icon_color(Color::Muted)
                    .size(ButtonSize::None)
                    .icon_size(IconSize::Small)
                    .on_click(cx.listener(move |pane, _, window, cx| {
                        pane.close_item_by_id(item_id, SaveIntent::Close, window, cx)
                            .detach_and_log_err(cx);
                    }))
                }
                .map(|this| {
                    if is_active {
                        let focus_handle = focus_handle.clone();
                        this.tooltip(move |window, cx| {
                            Tooltip::for_action_in(
                                end_slot_tooltip_text,
                                end_slot_action,
                                &window.focused(cx).unwrap_or_else(|| focus_handle.clone()),
                                cx,
                            )
                        })
                    } else {
                        this.tooltip(Tooltip::text(end_slot_tooltip_text))
                    }
                });
                this.end_slot(end_slot)
            })
            .child(
                h_flex()
                    .id(("pane-tab-content", ix))
                    .gap_1()
                    .children(if let Some(icon) = icon {
                        Some(icon)
                    } else if !capability.editable() {
                        Some(read_only_toggle(capability == Capability::Read).into_any_element())
                    } else {
                        None
                    })
                    .child(label)
                    .map(|this| match tab_tooltip_content {
                        Some(TabTooltipContent::Text(text)) => {
                            if capability.editable() {
                                this.tooltip(Tooltip::text(text))
                            } else {
                                this.tooltip(move |_, cx| {
                                    let text = text.clone();
                                    Tooltip::with_meta(text, None, "Read-Only Tab", cx)
                                })
                            }
                        }
                        Some(TabTooltipContent::Custom(element_fn)) => {
                            this.tooltip(move |window, cx| element_fn(window, cx))
                        }
                        None => this,
                    })
                    .when(capability == Capability::Read && has_file_icon, |this| {
                        this.child(read_only_toggle(true))
                    }),
            );

        let single_entry_to_resolve = (self.items[ix].buffer_kind(cx) == ItemBufferKind::Singleton)
            .then(|| self.items[ix].project_entry_ids(cx).get(0).copied())
            .flatten();

        let total_items = self.items.len();
        let has_multibuffer_items = self
            .items
            .iter()
            .any(|item| item.buffer_kind(cx) == ItemBufferKind::Multibuffer);
        let has_items_to_left = ix > 0;
        let has_items_to_right = ix < total_items - 1;
        let has_clean_items = self.items.iter().any(|item| !item.is_dirty(cx));
        let is_pinned = self.is_tab_pinned(ix);

        let pane = cx.entity().downgrade();
        let menu_context = item.item_focus_handle(cx);
        let item_handle = item.boxed_clone();

        right_click_menu(ix)
            .trigger(|_, _, _| tab)
            .menu(move |window, cx| {
                let pane = pane.clone();
                let menu_context = menu_context.clone();
                let extra_actions = item_handle.tab_extra_context_menu_actions(window, cx);
                ContextMenu::build(window, cx, move |mut menu, window, cx| {
                    let close_active_item_action = CloseActiveItem {
                        save_intent: None,
                        close_pinned: true,
                    };
                    let close_inactive_items_action = CloseOtherItems {
                        save_intent: None,
                        close_pinned: false,
                    };
                    let close_multibuffers_action = CloseMultibufferItems {
                        save_intent: None,
                        close_pinned: false,
                    };
                    let close_items_to_the_left_action = CloseItemsToTheLeft {
                        close_pinned: false,
                    };
                    let close_items_to_the_right_action = CloseItemsToTheRight {
                        close_pinned: false,
                    };
                    let close_clean_items_action = CloseCleanItems {
                        close_pinned: false,
                    };
                    let close_all_items_action = CloseAllItems {
                        save_intent: None,
                        close_pinned: false,
                    };
                    if let Some(pane) = pane.upgrade() {
                        menu = menu
                            .entry(
                                "Close",
                                Some(Box::new(close_active_item_action)),
                                window.handler_for(&pane, move |pane, window, cx| {
                                    pane.close_item_by_id(item_id, SaveIntent::Close, window, cx)
                                        .detach_and_log_err(cx);
                                }),
                            )
                            .item(ContextMenuItem::Entry(
                                ContextMenuEntry::new("Close Others")
                                    .action(Box::new(close_inactive_items_action.clone()))
                                    .disabled(total_items == 1)
                                    .handler(window.handler_for(&pane, move |pane, window, cx| {
                                        pane.close_other_items(
                                            &close_inactive_items_action,
                                            Some(item_id),
                                            window,
                                            cx,
                                        )
                                        .detach_and_log_err(cx);
                                    })),
                            ))
                            // We make this optional, instead of using disabled as to not overwhelm the context menu unnecessarily
                            .extend(has_multibuffer_items.then(|| {
                                ContextMenuItem::Entry(
                                    ContextMenuEntry::new("Close Multibuffers")
                                        .action(Box::new(close_multibuffers_action.clone()))
                                        .handler(window.handler_for(
                                            &pane,
                                            move |pane, window, cx| {
                                                pane.close_multibuffer_items(
                                                    &close_multibuffers_action,
                                                    window,
                                                    cx,
                                                )
                                                .detach_and_log_err(cx);
                                            },
                                        )),
                                )
                            }))
                            .separator()
                            .item(ContextMenuItem::Entry(
                                ContextMenuEntry::new("Close Left")
                                    .action(Box::new(close_items_to_the_left_action.clone()))
                                    .disabled(!has_items_to_left)
                                    .handler(window.handler_for(&pane, move |pane, window, cx| {
                                        pane.close_items_to_the_left_by_id(
                                            Some(item_id),
                                            &close_items_to_the_left_action,
                                            window,
                                            cx,
                                        )
                                        .detach_and_log_err(cx);
                                    })),
                            ))
                            .item(ContextMenuItem::Entry(
                                ContextMenuEntry::new("Close Right")
                                    .action(Box::new(close_items_to_the_right_action.clone()))
                                    .disabled(!has_items_to_right)
                                    .handler(window.handler_for(&pane, move |pane, window, cx| {
                                        pane.close_items_to_the_right_by_id(
                                            Some(item_id),
                                            &close_items_to_the_right_action,
                                            window,
                                            cx,
                                        )
                                        .detach_and_log_err(cx);
                                    })),
                            ))
                            .separator()
                            .item(ContextMenuItem::Entry(
                                ContextMenuEntry::new("Close Clean")
                                    .action(Box::new(close_clean_items_action.clone()))
                                    .disabled(!has_clean_items)
                                    .handler(window.handler_for(&pane, move |pane, window, cx| {
                                        pane.close_clean_items(
                                            &close_clean_items_action,
                                            window,
                                            cx,
                                        )
                                        .detach_and_log_err(cx)
                                    })),
                            ))
                            .entry(
                                "Close All",
                                Some(Box::new(close_all_items_action.clone())),
                                window.handler_for(&pane, move |pane, window, cx| {
                                    pane.close_all_items(&close_all_items_action, window, cx)
                                        .detach_and_log_err(cx)
                                }),
                            );

                        let pin_tab_entries = |menu: ContextMenu| {
                            menu.separator().map(|this| {
                                if is_pinned {
                                    this.entry(
                                        "Unpin Tab",
                                        Some(TogglePinTab.boxed_clone()),
                                        window.handler_for(&pane, move |pane, window, cx| {
                                            pane.unpin_tab_at(ix, window, cx);
                                        }),
                                    )
                                } else {
                                    this.entry(
                                        "Pin Tab",
                                        Some(TogglePinTab.boxed_clone()),
                                        window.handler_for(&pane, move |pane, window, cx| {
                                            pane.pin_tab_at(ix, window, cx);
                                        }),
                                    )
                                }
                            })
                        };

                        if capability != Capability::ReadOnly {
                            let read_only_label = if capability.editable() {
                                "Make Tab Read-Only"
                            } else {
                                "Make Tab Editable"
                            };
                            menu = menu.separator().entry(
                                read_only_label,
                                None,
                                window.handler_for(&pane, move |pane, window, cx| {
                                    if let Some(item) = pane.item_for_index(ix) {
                                        item.toggle_read_only(window, cx);
                                    }
                                }),
                            );
                        }

                        if let Some(entry) = single_entry_to_resolve {
                            let project_path = pane
                                .read(cx)
                                .item_for_entry(entry, cx)
                                .and_then(|item| item.project_path(cx));
                            let worktree = project_path.as_ref().and_then(|project_path| {
                                pane.read(cx)
                                    .project
                                    .upgrade()?
                                    .read(cx)
                                    .worktree_for_id(project_path.worktree_id, cx)
                            });
                            let has_relative_path = worktree.as_ref().is_some_and(|worktree| {
                                worktree
                                    .read(cx)
                                    .root_entry()
                                    .is_some_and(|entry| entry.is_dir())
                            });

                            let entry_abs_path = pane.read(cx).entry_abs_path(entry, cx);
                            let reveal_path = entry_abs_path.clone();
                            let parent_abs_path = entry_abs_path
                                .as_deref()
                                .and_then(|abs_path| Some(abs_path.parent()?.to_path_buf()));
                            let has_git_repo = project_path.as_ref().is_some_and(|project_path| {
                                pane.read(cx).project.upgrade().is_some_and(|project| {
                                    project
                                        .read(cx)
                                        .git_store()
                                        .read(cx)
                                        .repository_and_path_for_project_path(project_path, cx)
                                        .is_some()
                                })
                            });
                            let relative_path = project_path
                                .as_ref()
                                .map(|project_path| project_path.path.clone())
                                .filter(|_| has_relative_path);

                            let visible_in_project_panel = relative_path.is_some()
                                && worktree.is_some_and(|worktree| worktree.read(cx).is_visible());
                            let is_local = pane.read(cx).project.upgrade().is_some_and(|project| {
                                let project = project.read(cx);
                                project.is_local() || project.is_via_wsl_with_host_interop(cx)
                            });
                            let is_remote = pane
                                .read(cx)
                                .project
                                .upgrade()
                                .is_some_and(|project| project.read(cx).is_remote());

                            let entry_id = entry.to_proto();

                            menu = menu
                                .separator()
                                .when_some(entry_abs_path, |menu, abs_path| {
                                    menu.entry(
                                        "Copy Path",
                                        Some(Box::new(aagent_actions::workspace::CopyPath)),
                                        window.handler_for(&pane, move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                abs_path.to_string_lossy().into_owned(),
                                            ));
                                        }),
                                    )
                                })
                                .when_some(relative_path, |menu, relative_path| {
                                    menu.entry(
                                        "Copy Relative Path",
                                        Some(Box::new(aagent_actions::workspace::CopyRelativePath)),
                                        window.handler_for(&pane, move |this, _, cx| {
                                            let Some(project) = this.project.upgrade() else {
                                                return;
                                            };
                                            let path_style = project
                                                .update(cx, |project, cx| project.path_style(cx));
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                relative_path.display(path_style).to_string(),
                                            ));
                                        }),
                                    )
                                })
                                .when(has_git_repo, |menu| {
                                    menu.separator().when_some(
                                        project_path.clone(),
                                        |menu, project_path| {
                                            menu.entry(
                                                "Open File Permalink",
                                                Some(OpenFilePermalink.boxed_clone()),
                                                window.handler_for(&pane, {
                                                    let project_path = project_path.clone();
                                                    move |pane, window, cx| {
                                                        let Some(project) = pane.project.upgrade()
                                                        else {
                                                            return;
                                                        };
                                                        crate::open_file_permalink(
                                                            project,
                                                            project_path.clone(),
                                                            pane.workspace.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }),
                                            )
                                            .entry(
                                                "Copy File Permalink",
                                                Some(CopyFilePermalink.boxed_clone()),
                                                window.handler_for(
                                                    &pane,
                                                    move |pane, window, cx| {
                                                        let Some(project) = pane.project.upgrade()
                                                        else {
                                                            return;
                                                        };
                                                        crate::copy_file_permalink(
                                                            project,
                                                            project_path.clone(),
                                                            pane.workspace.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ),
                                            )
                                        },
                                    )
                                })
                                .when(is_local, |menu| {
                                    menu.when_some(reveal_path, |menu, reveal_path| {
                                        menu.separator().entry(
                                            ui::utils::reveal_in_file_manager_label(is_remote),
                                            Some(Box::new(
                                                aagent_actions::editor::RevealInFileManager,
                                            )),
                                            window.handler_for(&pane, move |pane, _, cx| {
                                                if let Some(project) = pane.project.upgrade() {
                                                    project.update(cx, |project, cx| {
                                                        project.reveal_path(&reveal_path, cx);
                                                    });
                                                } else {
                                                    cx.reveal_path(&reveal_path);
                                                }
                                            }),
                                        )
                                    })
                                })
                                .map(pin_tab_entries)
                                .when(visible_in_project_panel, |menu| {
                                    menu.entry(
                                        "Reveal In Project Panel",
                                        Some(Box::new(RevealInProjectPanel::default())),
                                        window.handler_for(&pane, move |pane, _, cx| {
                                            pane.project
                                                .update(cx, |_, cx| {
                                                    cx.emit(project::Event::RevealInProjectPanel(
                                                        ProjectEntryId::from_proto(entry_id),
                                                    ))
                                                })
                                                .ok();
                                        }),
                                    )
                                })
                                .when_some(parent_abs_path, |menu, parent_abs_path| {
                                    menu.entry(
                                        "Open in Terminal",
                                        Some(Box::new(OpenInTerminal)),
                                        window.handler_for(&pane, move |_, window, cx| {
                                            window.dispatch_action(
                                                OpenTerminal {
                                                    working_directory: parent_abs_path.clone(),
                                                    local: false,
                                                }
                                                .boxed_clone(),
                                                cx,
                                            );
                                        }),
                                    )
                                });
                        } else {
                            menu = menu.map(pin_tab_entries);
                        }
                    };

                    // Add custom item-specific actions
                    if !extra_actions.is_empty() {
                        menu = menu.separator();
                        for (label, action) in extra_actions {
                            menu = menu.action(label, action);
                        }
                    }

                    menu.context(menu_context)
                })
            })
    }

    pub(super) fn render_tab_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> AnyElement {
        if self.workspace.upgrade().is_none() {
            return gpui::Empty.into_any();
        }

        let focus_handle = self.focus_handle.clone();

        let navigate_backward = IconButton::new("navigate_backward", IconName::ArrowLeft)
            .icon_size(IconSize::Small)
            .on_click({
                let entity = cx.entity();
                move |_, window, cx| {
                    entity.update(cx, |pane, cx| {
                        pane.navigate_backward(&Default::default(), window, cx)
                    })
                }
            })
            .disabled(!self.can_navigate_backward())
            .tooltip({
                let focus_handle = focus_handle.clone();
                move |window, cx| {
                    Tooltip::for_action_in(
                        "Go Back",
                        &GoBack,
                        &window.focused(cx).unwrap_or_else(|| focus_handle.clone()),
                        cx,
                    )
                }
            });

        let navigate_forward = IconButton::new("navigate_forward", IconName::ArrowRight)
            .icon_size(IconSize::Small)
            .on_click({
                let entity = cx.entity();
                move |_, window, cx| {
                    entity.update(cx, |pane, cx| {
                        pane.navigate_forward(&Default::default(), window, cx)
                    })
                }
            })
            .disabled(!self.can_navigate_forward())
            .tooltip({
                let focus_handle = focus_handle.clone();
                move |window, cx| {
                    Tooltip::for_action_in(
                        "Go Forward",
                        &GoForward,
                        &window.focused(cx).unwrap_or_else(|| focus_handle.clone()),
                        cx,
                    )
                }
            });

        let mut tab_items = self
            .items
            .iter()
            .enumerate()
            .zip(tab_details(&self.items, window, cx))
            .map(|((ix, item), detail)| {
                self.render_tab(ix, &**item, detail, &focus_handle, window, cx)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let tab_count = tab_items.len();
        if self.is_tab_pinned(tab_count) {
            log::warn!(
                "Pinned tab count ({}) exceeds actual tab count ({}). \
            This should not happen. If possible, add reproduction steps, \
            in a comment, to https://github.com/zed-industries/zed/issues/33342",
                self.pinned_tab_count,
                tab_count
            );
            self.pinned_tab_count = tab_count;
        }
        let unpinned_tabs = tab_items.split_off(self.pinned_tab_count);
        let pinned_tabs = tab_items;

        let tab_bar_settings = TabBarSettings::get_global(cx);
        let use_separate_rows = tab_bar_settings.show_pinned_tabs_in_separate_row;

        if use_separate_rows && !pinned_tabs.is_empty() && !unpinned_tabs.is_empty() {
            self.render_two_row_tab_bar(
                pinned_tabs,
                unpinned_tabs,
                tab_count,
                navigate_backward,
                navigate_forward,
                window,
                cx,
            )
        } else {
            self.render_single_row_tab_bar(
                pinned_tabs,
                unpinned_tabs,
                tab_count,
                navigate_backward,
                navigate_forward,
                window,
                cx,
            )
        }
    }

    pub(super) fn configure_tab_bar_start(
        &mut self,
        tab_bar: TabBar,
        navigate_backward: IconButton,
        navigate_forward: IconButton,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> TabBar {
        tab_bar
            .when(
                self.display_nav_history_buttons.unwrap_or_default(),
                |tab_bar| {
                    tab_bar
                        .start_child(navigate_backward)
                        .start_child(navigate_forward)
                },
            )
            .map(|tab_bar| {
                if self.show_tab_bar_buttons {
                    let render_tab_buttons = self.render_tab_bar_buttons.clone();
                    let (left_children, right_children) = render_tab_buttons(self, window, cx);
                    tab_bar
                        .start_children(left_children)
                        .end_children(right_children)
                } else {
                    tab_bar
                }
            })
    }

    pub(super) fn render_single_row_tab_bar(
        &mut self,
        pinned_tabs: Vec<AnyElement>,
        unpinned_tabs: Vec<AnyElement>,
        tab_count: usize,
        navigate_backward: IconButton,
        navigate_forward: IconButton,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> AnyElement {
        let tab_bar = self
            .configure_tab_bar_start(
                TabBar::new("tab_bar"),
                navigate_backward,
                navigate_forward,
                window,
                cx,
            )
            .children(pinned_tabs.len().ne(&0).then(|| {
                let max_scroll = self.tab_bar_scroll_handle.max_offset().x;
                // We need to check both because offset returns delta values even when the scroll handle is not scrollable
                let is_scrolled = self.tab_bar_scroll_handle.offset().x < px(0.);
                // Avoid flickering when max_offset is very small (< 2px).
                // The border adds 1-2px which can push max_offset back to 0, creating a loop.
                let is_scrollable = max_scroll > px(2.0);
                let has_active_unpinned_tab = self.active_item_index >= self.pinned_tab_count;
                h_flex()
                    .children(pinned_tabs)
                    .when(is_scrollable && is_scrolled, |this| {
                        this.when(has_active_unpinned_tab, |this| this.border_r_2())
                            .when(!has_active_unpinned_tab, |this| this.border_r_1())
                            .border_color(cx.theme().colors().border)
                    })
            }))
            .child(self.render_unpinned_tabs_container(unpinned_tabs, tab_count, cx));
        tab_bar.into_any_element()
    }

    pub(super) fn render_two_row_tab_bar(
        &mut self,
        pinned_tabs: Vec<AnyElement>,
        unpinned_tabs: Vec<AnyElement>,
        tab_count: usize,
        navigate_backward: IconButton,
        navigate_forward: IconButton,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> AnyElement {
        let pinned_tab_bar = self
            .configure_tab_bar_start(
                TabBar::new("pinned_tab_bar"),
                navigate_backward,
                navigate_forward,
                window,
                cx,
            )
            .child(
                h_flex()
                    .id("pinned_tabs_row")
                    .debug_selector(|| "pinned_tabs_row".into())
                    .overflow_x_scroll()
                    .w_full()
                    .children(pinned_tabs)
                    .child(self.render_pinned_tab_bar_drop_target(cx)),
            );
        v_flex()
            .w_full()
            .flex_none()
            .child(pinned_tab_bar)
            .child(
                TabBar::new("unpinned_tab_bar").child(self.render_unpinned_tabs_container(
                    unpinned_tabs,
                    tab_count,
                    cx,
                )),
            )
            .into_any_element()
    }

    pub(super) fn render_unpinned_tabs_container(
        &mut self,
        unpinned_tabs: Vec<AnyElement>,
        tab_count: usize,
        cx: &mut Context<Pane>,
    ) -> impl IntoElement {
        h_flex()
            .id("unpinned tabs")
            .overflow_x_scroll()
            .w_full()
            .track_scroll(&self.tab_bar_scroll_handle)
            .on_scroll_wheel(cx.listener(|this, _, _, _| {
                this.suppress_scroll = true;
            }))
            .children(unpinned_tabs)
            .child(self.render_tab_bar_drop_target(tab_count, cx))
    }

    pub(super) fn render_tab_bar_drop_target(
        &self,
        tab_count: usize,
        cx: &mut Context<Pane>,
    ) -> impl IntoElement {
        div()
            .id("tab_bar_drop_target")
            .min_w_6()
            .h(Tab::container_height(cx))
            .flex_grow_1()
            // HACK: This empty child is currently necessary to force the drop target to appear
            // despite us setting a min width above.
            .child("")
            .drag_over::<DraggedTab>(|bar, _, _, cx| {
                bar.bg(cx.theme().colors().drop_target_background)
            })
            .drag_over::<DraggedSelection>(|bar, _, _, cx| {
                bar.bg(cx.theme().colors().drop_target_background)
            })
            .on_drop(
                cx.listener(move |this, dragged_tab: &DraggedTab, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_tab_drop(dragged_tab, this.items.len(), false, window, cx)
                }),
            )
            .on_drop(
                cx.listener(move |this, selection: &DraggedSelection, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_project_entry_drop(
                        &selection.active_selection.entry_id,
                        Some(tab_count),
                        window,
                        cx,
                    )
                }),
            )
            .on_drop(cx.listener(move |this, paths, window, cx| {
                this.drag_split_direction = None;
                this.handle_external_paths_drop(paths, window, cx)
            }))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    window.dispatch_action(this.double_click_dispatch_action.boxed_clone(), cx);
                }
            }))
    }

    pub(super) fn render_pinned_tab_bar_drop_target(
        &self,
        cx: &mut Context<Pane>,
    ) -> impl IntoElement {
        div()
            .id("pinned_tabs_border")
            .debug_selector(|| "pinned_tabs_border".into())
            .min_w_6()
            .h(Tab::container_height(cx))
            .flex_grow_1()
            .border_l_1()
            .border_color(cx.theme().colors().border)
            // HACK: This empty child is currently necessary to force the drop target to appear
            // despite us setting a min width above.
            .child("")
            .drag_over::<DraggedTab>(|bar, _, _, cx| {
                bar.bg(cx.theme().colors().drop_target_background)
            })
            .drag_over::<DraggedSelection>(|bar, _, _, cx| {
                bar.bg(cx.theme().colors().drop_target_background)
            })
            .on_drop(
                cx.listener(move |this, dragged_tab: &DraggedTab, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_pinned_tab_bar_drop(dragged_tab, window, cx)
                }),
            )
            .on_drop(
                cx.listener(move |this, selection: &DraggedSelection, window, cx| {
                    this.drag_split_direction = None;
                    this.handle_project_entry_drop(
                        &selection.active_selection.entry_id,
                        Some(this.pinned_tab_count),
                        window,
                        cx,
                    )
                }),
            )
            .on_drop(cx.listener(move |this, paths, window, cx| {
                this.drag_split_direction = None;
                this.handle_external_paths_drop(paths, window, cx)
            }))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    window.dispatch_action(this.double_click_dispatch_action.boxed_clone(), cx);
                }
            }))
    }

    pub fn render_menu_overlay(menu: &Entity<ContextMenu>) -> Div {
        div().absolute().bottom_0().right_0().size_0().child(
            deferred(anchored().anchor(Anchor::TopRight).child(menu.clone())).with_priority(1),
        )
    }
}
