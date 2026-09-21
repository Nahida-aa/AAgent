use std::cmp;
use std::sync::Arc;

use collections::HashMap;
use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Window, div};
use ui::prelude::*;
use workspace::{
    ActivateNextPane, ActivatePane, ActivatePaneDown, ActivatePaneLeft, ActivatePaneRight,
    ActivatePaneUp, ActivatePreviousPane, MoveItemToPane, MoveItemToPaneInDirection, MovePaneDown,
    MovePaneLeft, MovePaneRight, MovePaneUp, SplitDirection, SwapPaneDown, SwapPaneLeft,
    SwapPaneRight, SwapPaneUp, Workspace, dock::Panel, move_active_item,
};

use super::TerminalPanel;

impl Render for TerminalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registrar = cx
            .try_global::<workspace::PaneSearchBarCallbacks>()
            .map(|callbacks| {
                (callbacks.wrap_div_with_search_actions)(div(), self.active_pane.clone())
            })
            .unwrap_or_else(div);
        let no_items_in_panes = self
            .center
            .panes()
            .into_iter()
            .all(|pane| pane.read(cx).items_len() == 0);
        let waiting_for_terminals = self.restoring || self.pending_terminals_to_add > 0;
        let restoring_placeholder = (waiting_for_terminals && no_items_in_panes).then(|| {
            let label = if self.restoring {
                "Restoring terminals…"
            } else {
                "Starting terminal…"
            };
            h_flex()
                .absolute()
                .inset_0()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::ArrowCircle)
                        .color(Color::Muted)
                        .size(IconSize::Small)
                        .with_rotate_animation(2),
                )
                .child(Label::new(label).color(Color::Muted))
        });
        self.workspace
            .update(cx, |workspace, cx| {
                registrar
                    .track_focus(&self.focus_handle)
                    .size_full()
                    .relative()
                    .child(self.center.render(
                        workspace.zoomed_item(),
                        None,
                        &workspace::PaneRenderContext {
                            follower_states: &HashMap::default(),
                            active_call: workspace.active_call(),
                            active_pane: &self.active_pane,
                            app_state: workspace.app_state(),
                            project: workspace.project(),
                            workspace: &workspace.weak_handle(),
                        },
                        window,
                        cx,
                    ))
                    .children(restoring_placeholder)
            })
            .ok()
            .map(|div| {
                div.on_action({
                    cx.listener(|terminal_panel, _: &ActivatePaneLeft, window, cx| {
                        terminal_panel.activate_pane_in_direction(SplitDirection::Left, window, cx);
                    })
                })
                .on_action({
                    cx.listener(|terminal_panel, _: &ActivatePaneRight, window, cx| {
                        terminal_panel.activate_pane_in_direction(
                            SplitDirection::Right,
                            window,
                            cx,
                        );
                    })
                })
                .on_action({
                    cx.listener(|terminal_panel, _: &ActivatePaneUp, window, cx| {
                        terminal_panel.activate_pane_in_direction(SplitDirection::Up, window, cx);
                    })
                })
                .on_action({
                    cx.listener(|terminal_panel, _: &ActivatePaneDown, window, cx| {
                        terminal_panel.activate_pane_in_direction(SplitDirection::Down, window, cx);
                    })
                })
                .on_action(
                    cx.listener(|terminal_panel, _action: &ActivateNextPane, window, cx| {
                        let panes = terminal_panel.center.panes();
                        if let Some(ix) = panes
                            .iter()
                            .position(|pane| **pane == terminal_panel.active_pane)
                        {
                            let next_ix = (ix + 1) % panes.len();
                            window.focus(&panes[next_ix].focus_handle(cx), cx);
                        }
                    }),
                )
                .on_action(cx.listener(
                    |terminal_panel, _action: &ActivatePreviousPane, window, cx| {
                        let panes = terminal_panel.center.panes();
                        if let Some(ix) = panes
                            .iter()
                            .position(|pane| **pane == terminal_panel.active_pane)
                        {
                            let prev_ix = cmp::min(ix.wrapping_sub(1), panes.len() - 1);
                            window.focus(&panes[prev_ix].focus_handle(cx), cx);
                        }
                    },
                ))
                .on_action(
                    cx.listener(|terminal_panel, action: &ActivatePane, window, cx| {
                        let panes = terminal_panel.center.panes();
                        if let Some(&pane) = panes.get(action.0) {
                            window.focus(&pane.read(cx).focus_handle(cx), cx);
                        } else {
                            let future =
                                terminal_panel.new_pane_with_active_terminal(true, window, cx);
                            cx.spawn_in(window, async move |terminal_panel, cx| {
                                if let Some(new_pane) = future.await {
                                    _ = terminal_panel.update_in(
                                        cx,
                                        |terminal_panel, window, cx| {
                                            terminal_panel.center.split(
                                                &terminal_panel.active_pane,
                                                &new_pane,
                                                SplitDirection::Right,
                                                cx,
                                            );
                                            let new_pane = new_pane.read(cx);
                                            window.focus(&new_pane.focus_handle(cx), cx);
                                        },
                                    );
                                }
                            })
                            .detach();
                        }
                    }),
                )
                .on_action(cx.listener(|terminal_panel, _: &SwapPaneLeft, _, cx| {
                    terminal_panel.swap_pane_in_direction(SplitDirection::Left, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &SwapPaneRight, _, cx| {
                    terminal_panel.swap_pane_in_direction(SplitDirection::Right, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &SwapPaneUp, _, cx| {
                    terminal_panel.swap_pane_in_direction(SplitDirection::Up, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &SwapPaneDown, _, cx| {
                    terminal_panel.swap_pane_in_direction(SplitDirection::Down, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &MovePaneLeft, _, cx| {
                    terminal_panel.move_pane_to_border(SplitDirection::Left, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &MovePaneRight, _, cx| {
                    terminal_panel.move_pane_to_border(SplitDirection::Right, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &MovePaneUp, _, cx| {
                    terminal_panel.move_pane_to_border(SplitDirection::Up, cx);
                }))
                .on_action(cx.listener(|terminal_panel, _: &MovePaneDown, _, cx| {
                    terminal_panel.move_pane_to_border(SplitDirection::Down, cx);
                }))
                .on_action(
                    cx.listener(|terminal_panel, action: &MoveItemToPane, window, cx| {
                        let Some(&target_pane) =
                            terminal_panel.center.panes().get(action.destination)
                        else {
                            return;
                        };
                        move_active_item(
                            &terminal_panel.active_pane,
                            target_pane,
                            action.focus,
                            true,
                            window,
                            cx,
                        );
                    }),
                )
                .on_action(cx.listener(
                    |terminal_panel, action: &MoveItemToPaneInDirection, window, cx| {
                        let source_pane = &terminal_panel.active_pane;
                        if let Some(destination_pane) = terminal_panel
                            .center
                            .find_pane_in_direction(source_pane, action.direction, cx)
                        {
                            move_active_item(
                                source_pane,
                                destination_pane,
                                action.focus,
                                true,
                                window,
                                cx,
                            );
                        };
                    },
                ))
            })
            .unwrap_or_else(|| div())
    }
}
