use gpui::{Context, Entity, Subscription, WeakEntity, Window};
use terminal::{Event, MaybeNavigationTarget, Terminal};
use workspace::Workspace;

use super::TerminalView;
use super::hover::HoverTarget;
use super::lifecycle::*; // 若 hover_tooltip_update 等字段
use crate::terminal_path_like_target::{hover_path_like_target, open_path_like_target};

pub(super) fn subscribe_for_terminal_events(
    terminal: &Entity<Terminal>,
    workspace: WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut Context<TerminalView>,
) -> Vec<Subscription> {
    let terminal_subscription = cx.observe(terminal, |_, _, cx| cx.notify());
    let mut previous_cwd = None;
    let terminal_events_subscription = cx.subscribe_in(
        terminal,
        window,
        move |terminal_view, terminal, event, window, cx| {
            let current_cwd = terminal.read(cx).working_directory();
            if current_cwd != previous_cwd {
                previous_cwd = current_cwd;
                terminal_view.needs_serialize = true;
            }

            match event {
                Event::Wakeup => {
                    cx.notify();
                    window.invalidate_character_coordinates();
                    cx.emit(Event::Wakeup);
                    cx.emit(ItemEvent::UpdateTab);
                    cx.emit(SearchEvent::MatchesInvalidated);
                }

                Event::Bell => {
                    terminal_view.has_bell = true;
                    if let TerminalBell::System = TerminalSettings::get_global(cx).bell {
                        window.play_system_bell();
                    }
                    cx.emit(Event::Wakeup);
                }

                Event::BlinkChanged(blinking) => {
                    terminal_view.blinking_terminal_enabled = *blinking;

                    // If in terminal-controlled mode and focused, update blink manager
                    if matches!(
                        TerminalSettings::get_global(cx).blinking,
                        TerminalBlink::TerminalControlled
                    ) && terminal_view.focus_handle.is_focused(window)
                    {
                        terminal_view.blink_manager.update(cx, |manager, cx| {
                            if *blinking {
                                manager.enable(cx);
                            } else {
                                manager.disable(cx);
                            }
                        });
                    }
                }

                Event::TitleChanged => {
                    cx.emit(ItemEvent::UpdateTab);
                }

                Event::NewNavigationTarget(maybe_navigation_target) => {
                    match maybe_navigation_target
                        .as_ref()
                        .zip(terminal.read(cx).last_content.last_hovered_word.as_ref())
                    {
                        Some((MaybeNavigationTarget::Url(url), hovered_word)) => {
                            if Some(hovered_word)
                                != terminal_view
                                    .hover
                                    .as_ref()
                                    .map(|hover| &hover.hovered_word)
                            {
                                terminal_view.hover = Some(HoverTarget {
                                    tooltip: url.clone(),
                                    hovered_word: hovered_word.clone(),
                                });
                                terminal_view.hover_tooltip_update = Task::ready(());
                                cx.notify();
                            }
                        }
                        Some((MaybeNavigationTarget::PathLike(path_like_target), hovered_word)) => {
                            if Some(hovered_word)
                                != terminal_view
                                    .hover
                                    .as_ref()
                                    .map(|hover| &hover.hovered_word)
                            {
                                terminal_view.hover = None;
                                terminal_view.hover_tooltip_update = hover_path_like_target(
                                    &workspace,
                                    hovered_word.clone(),
                                    path_like_target,
                                    cx,
                                );
                                cx.notify();
                            }
                        }
                        None => {
                            terminal_view.hover = None;
                            terminal_view.hover_tooltip_update = Task::ready(());
                            cx.notify();
                        }
                    }
                }

                Event::Open(maybe_navigation_target) => match maybe_navigation_target {
                    MaybeNavigationTarget::Url(url) => cx.open_url(url),
                    MaybeNavigationTarget::PathLike(path_like_target) => open_path_like_target(
                        &workspace,
                        terminal_view,
                        path_like_target,
                        window,
                        cx,
                    ),
                },
                Event::BreadcrumbsChanged => cx.emit(ItemEvent::UpdateBreadcrumbs),
                Event::CloseTerminal => cx.emit(ItemEvent::CloseItem),
                Event::SelectionsChanged => {
                    window.invalidate_character_coordinates();
                    cx.emit(SearchEvent::ActiveMatchChanged)
                }
            }
        },
    );
    vec![terminal_subscription, terminal_events_subscription]
}
