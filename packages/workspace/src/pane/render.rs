use super::actions::{
    ActivateItem, ActivateLastItem, ActivateNextItem, ActivatePreviousItem, AlternateFile,
    CloseActiveItem, CloseAllItems, CloseCleanItems, CloseItemsToTheLeft, CloseItemsToTheRight,
    CloseMultibufferItems, CloseOtherItems, DeploySearch, GoBack, GoForward, GoToNewerTag,
    GoToOlderTag, JoinAll, JoinIntoNext, RevealInProjectPanel, SplitAndMoveDown, SplitAndMoveLeft,
    SplitAndMoveRight, SplitAndMoveUp, SplitDown, SplitHorizontal, SplitLeft, SplitRight, SplitUp,
    SplitVertical, SwapItemLeft, SwapItemRight, TogglePinTab, TogglePreviewTab, UnpinAllTabs,
};
use super::{DraggedSelection, DraggedTab, Event, Pane, SplitMode};
use crate::{
    SplitDirection,
    item::{
        ActivateOnClose, ClosePosition, Item, ItemBufferKind, ItemHandle, ItemSettings,
        PreviewTabsSettings, ProjectItemKind, SaveOptions, ShowCloseButton, ShowDiagnostics,
        TabContentParams, TabTooltipContent, WeakItemHandle,
    },
};
use anyhow::Result;

use project::{DirectoryLister, Project, ProjectEntryId, ProjectPath, WorktreeId};
use theme_settings::ThemeSettings;
use ui::{
    ContextMenu, ContextMenuEntry, ContextMenuItem, DecoratedIcon, IconButtonShape, IconDecoration,
    IconDecorationKind, Indicator, PopoverMenu, PopoverMenuHandle, Tab, TabBar, TabPosition,
    Tooltip, prelude::*, right_click_menu,
};
//
use gpui::{
    Action, Anchor, AnyElement, App, AsyncWindowContext, ClickEvent, ClipboardItem, Context, Div,
    DragMoveEvent, Entity, EntityId, EventEmitter, ExternalPaths, FocusHandle, FocusOutEvent,
    Focusable, KeyContext, MouseButton, NavigationDirection, Pixels, Point, PromptLevel, Render,
    ScrollHandle, Subscription, Task, TaskExt, WeakEntity, WeakFocusHandle, Window, actions,
    anchored, deferred, prelude::*,
};
use util::{
    ResultExt, debug_panic, markdown::MarkdownInlineCode, maybe, paths::PathStyle,
    serde::default_true, truncate_and_remove_front,
};

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add("Pane");
        if self.active_item().is_none() {
            key_context.add("EmptyPane");
        }

        self.toolbar
            .read(cx)
            .contribute_context(&mut key_context, cx);

        let should_display_tab_bar = self.should_display_tab_bar.clone();
        let display_tab_bar = should_display_tab_bar(window, cx);
        let Some(project) = self.project.upgrade() else {
            return div().track_focus(&self.focus_handle(cx));
        };
        // WSL remotes accept dropped host files too, since their paths can be translated with `wslpath`; see `Pane::handle_external_paths_drop`.
        let accepts_external_paths = {
            let project = project.read(cx);
            project.is_local() || project.is_via_wsl(cx)
        };

        v_flex()
            .key_context(key_context)
            .track_focus(&self.focus_handle(cx))
            .size_full()
            .flex_none()
            .overflow_hidden()
            .on_action(cx.listener(|pane, split: &SplitLeft, window, cx| {
                pane.split(SplitDirection::Left, split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, split: &SplitUp, window, cx| {
                pane.split(SplitDirection::Up, split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, split: &SplitHorizontal, window, cx| {
                pane.split(SplitDirection::horizontal(cx), split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, split: &SplitVertical, window, cx| {
                pane.split(SplitDirection::vertical(cx), split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, split: &SplitRight, window, cx| {
                pane.split(SplitDirection::Right, split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, split: &SplitDown, window, cx| {
                pane.split(SplitDirection::Down, split.mode, window, cx)
            }))
            .on_action(cx.listener(|pane, _: &SplitAndMoveUp, window, cx| {
                pane.split(SplitDirection::Up, SplitMode::MovePane, window, cx)
            }))
            .on_action(cx.listener(|pane, _: &SplitAndMoveDown, window, cx| {
                pane.split(SplitDirection::Down, SplitMode::MovePane, window, cx)
            }))
            .on_action(cx.listener(|pane, _: &SplitAndMoveLeft, window, cx| {
                pane.split(SplitDirection::Left, SplitMode::MovePane, window, cx)
            }))
            .on_action(cx.listener(|pane, _: &SplitAndMoveRight, window, cx| {
                pane.split(SplitDirection::Right, SplitMode::MovePane, window, cx)
            }))
            .on_action(cx.listener(|_, _: &JoinIntoNext, _, cx| {
                cx.emit(Event::JoinIntoNext);
            }))
            .on_action(cx.listener(|_, _: &JoinAll, _, cx| {
                cx.emit(Event::JoinAll);
            }))
            .on_action(cx.listener(Pane::toggle_zoom))
            .on_action(cx.listener(Pane::zoom_in))
            .on_action(cx.listener(Pane::zoom_out))
            .on_action(cx.listener(Self::navigate_backward))
            .on_action(cx.listener(Self::navigate_forward))
            .on_action(cx.listener(Self::go_to_older_tag))
            .on_action(cx.listener(Self::go_to_newer_tag))
            .on_action(
                cx.listener(|pane: &mut Pane, action: &ActivateItem, window, cx| {
                    pane.activate_item(
                        action.0.min(pane.items.len().saturating_sub(1)),
                        true,
                        true,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(cx.listener(Self::alternate_file))
            .on_action(cx.listener(Self::activate_last_item))
            .on_action(cx.listener(Self::activate_previous_item))
            .on_action(cx.listener(Self::activate_next_item))
            .on_action(cx.listener(Self::swap_item_left))
            .on_action(cx.listener(Self::swap_item_right))
            .on_action(cx.listener(Self::toggle_pin_tab))
            .on_action(cx.listener(Self::unpin_all_tabs))
            .when(PreviewTabsSettings::get_global(cx).enabled, |this| {
                this.on_action(
                    cx.listener(|pane: &mut Pane, _: &TogglePreviewTab, window, cx| {
                        if let Some(active_item_id) = pane.active_item().map(|i| i.item_id()) {
                            if pane.is_active_preview_item(active_item_id) {
                                pane.unpreview_item_if_preview(active_item_id);
                            } else {
                                pane.replace_preview_item_id(active_item_id, window, cx);
                            }
                        }
                    }),
                )
            })
            .on_action(
                cx.listener(|pane: &mut Self, action: &CloseActiveItem, window, cx| {
                    pane.close_active_item(action, window, cx)
                        .detach_and_log_err(cx)
                }),
            )
            .on_action(
                cx.listener(|pane: &mut Self, action: &CloseOtherItems, window, cx| {
                    pane.close_other_items(action, None, window, cx)
                        .detach_and_log_err(cx);
                }),
            )
            .on_action(
                cx.listener(|pane: &mut Self, action: &CloseCleanItems, window, cx| {
                    pane.close_clean_items(action, window, cx)
                        .detach_and_log_err(cx)
                }),
            )
            .on_action(cx.listener(
                |pane: &mut Self, action: &CloseItemsToTheLeft, window, cx| {
                    pane.close_items_to_the_left_by_id(None, action, window, cx)
                        .detach_and_log_err(cx)
                },
            ))
            .on_action(cx.listener(
                |pane: &mut Self, action: &CloseItemsToTheRight, window, cx| {
                    pane.close_items_to_the_right_by_id(None, action, window, cx)
                        .detach_and_log_err(cx)
                },
            ))
            .on_action(
                cx.listener(|pane: &mut Self, action: &CloseAllItems, window, cx| {
                    pane.close_all_items(action, window, cx)
                        .detach_and_log_err(cx)
                }),
            )
            .on_action(cx.listener(
                |pane: &mut Self, action: &CloseMultibufferItems, window, cx| {
                    pane.close_multibuffer_items(action, window, cx)
                        .detach_and_log_err(cx)
                },
            ))
            .on_action(cx.listener(
                |pane: &mut Self, action: &RevealInProjectPanel, _window, cx| {
                    let entry_id = action.entry_id.map(ProjectEntryId::from_proto);
                    let active_project_path =
                        pane.active_item().and_then(|item| item.project_path(cx));

                    pane.project
                        .update(cx, |project, cx| {
                            let entry_id = entry_id.or_else(|| {
                                active_project_path
                                    .as_ref()
                                    .and_then(|path| project.entry_for_path(path, cx))
                                    .map(|entry| entry.id)
                            });
                            if let Some(entry_id) = entry_id
                                && project
                                    .worktree_for_entry(entry_id, cx)
                                    .is_some_and(|worktree| worktree.read(cx).is_visible())
                            {
                                return cx.emit(project::Event::RevealInProjectPanel(entry_id));
                            }

                            // When no entry is found, which is the case when
                            // working with an unsaved buffer, or the worktree
                            // is not visible, for example, a file that doesn't
                            // belong to an open project, we can't reveal the
                            // entry but we still want to activate the project
                            // panel.
                            cx.emit(project::Event::ActivateProjectPanel);
                        })
                        .log_err();
                },
            ))
            .on_action(cx.listener(|_, _: &menu::Cancel, window, cx| {
                if cx.stop_active_drag(window) {
                } else {
                    cx.propagate();
                }
            }))
            .when(self.active_item().is_some() && display_tab_bar, |pane| {
                pane.child((self.render_tab_bar.clone())(self, window, cx))
            })
            .child({
                let has_worktrees = project.read(cx).visible_worktrees(cx).next().is_some();
                // main content
                div()
                    .flex_1()
                    .relative()
                    .group("")
                    .overflow_hidden()
                    .on_drag_move::<DraggedTab>(cx.listener(Self::handle_drag_move))
                    .on_drag_move::<DraggedSelection>(cx.listener(Self::handle_drag_move))
                    .when(accepts_external_paths, |div| {
                        div.on_drag_move::<ExternalPaths>(cx.listener(Self::handle_drag_move))
                    })
                    .map(|div| {
                        if let Some(item) = self.active_item() {
                            div.id("pane_placeholder")
                                .v_flex()
                                .size_full()
                                .overflow_hidden()
                                .child(self.toolbar.clone())
                                .child(item.to_any_view())
                        } else {
                            let placeholder = div
                                .id("pane_placeholder")
                                .h_flex()
                                .size_full()
                                .justify_center()
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        if event.click_count() == 2 {
                                            window.dispatch_action(
                                                this.double_click_dispatch_action.boxed_clone(),
                                                cx,
                                            );
                                        }
                                    },
                                ));
                            if has_worktrees || !self.should_display_welcome_page {
                                placeholder
                            } else {
                                if self.welcome_page.is_none() {
                                    let workspace = self.workspace.clone();
                                    self.welcome_page = Some(cx.new(|cx| {
                                        crate::welcome::WelcomePage::new(
                                            workspace, true, window, cx,
                                        )
                                    }));
                                }
                                placeholder.child(self.welcome_page.clone().unwrap())
                            }
                        }
                        .focus_follows_mouse(self.focus_follows_mouse, cx)
                    })
                    .child(
                        // drag target
                        div()
                            .invisible()
                            .absolute()
                            .bg(cx.theme().colors().drop_target_background)
                            .group_drag_over::<DraggedTab>("", |style| style.visible())
                            .group_drag_over::<DraggedSelection>("", |style| style.visible())
                            .when(accepts_external_paths, |div| {
                                div.group_drag_over::<ExternalPaths>("", |style| style.visible())
                            })
                            .when_some(self.can_drop_predicate.clone(), |this, p| {
                                this.can_drop(move |a, window, cx| p(a, window, cx))
                            })
                            .on_drop(cx.listener(move |this, dragged_tab, window, cx| {
                                this.handle_tab_drop(
                                    dragged_tab,
                                    this.active_item_index(),
                                    true,
                                    window,
                                    cx,
                                )
                            }))
                            .on_drop(cx.listener(
                                move |this, selection: &DraggedSelection, window, cx| {
                                    this.handle_dragged_selection_drop(selection, None, window, cx)
                                },
                            ))
                            .on_drop(cx.listener(move |this, paths, window, cx| {
                                this.handle_external_paths_drop(paths, window, cx)
                            }))
                            .map(|div| {
                                let size = DefiniteLength::Fraction(0.5);
                                match self.drag_split_direction {
                                    None => div.top_0().right_0().bottom_0().left_0(),
                                    Some(SplitDirection::Up) => {
                                        div.top_0().left_0().right_0().h(size)
                                    }
                                    Some(SplitDirection::Down) => {
                                        div.left_0().bottom_0().right_0().h(size)
                                    }
                                    Some(SplitDirection::Left) => {
                                        div.top_0().left_0().bottom_0().w(size)
                                    }
                                    Some(SplitDirection::Right) => {
                                        div.top_0().bottom_0().right_0().w(size)
                                    }
                                }
                            }),
                    )
            })
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|pane, _, window, cx| {
                    if let Some(workspace) = pane.workspace.upgrade() {
                        let pane = cx.entity().downgrade();
                        window.defer(cx, move |window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                workspace.go_back(pane, window, cx).detach_and_log_err(cx)
                            })
                        })
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|pane, _, window, cx| {
                    if let Some(workspace) = pane.workspace.upgrade() {
                        let pane = cx.entity().downgrade();
                        window.defer(cx, move |window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                workspace
                                    .go_forward(pane, window, cx)
                                    .detach_and_log_err(cx)
                            })
                        })
                    }
                }),
            )
    }
}

impl Render for DraggedTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = ThemeSettings::get_global(cx).ui_font.clone();
        let label = self.item.tab_content(
            TabContentParams {
                detail: Some(self.detail),
                selected: false,
                preview: false,
                deemphasized: false,
                max_title_len: None,
                truncate_title_middle: false,
            },
            window,
            cx,
        );
        let icon =
            self.pane
                .read(cx)
                .tab_icon_element(self.item.as_ref(), self.is_active, window, cx);
        Tab::new("")
            .toggle_state(self.is_active)
            .children(icon)
            .child(label)
            .render(window, cx)
            .font(ui_font)
    }
}
