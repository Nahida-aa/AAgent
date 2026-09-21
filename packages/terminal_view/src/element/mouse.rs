use gpui::{
    App, Context, DispatchPhase, Entity, FocusHandle, Hitbox, MouseButton, MouseMoveEvent, Pixels,
    Window,
};
use terminal::{Modes, Terminal};

use crate::{ContentMode, TerminalMode, TerminalView};

use super::TerminalElement;

impl TerminalElement {
    pub(super) fn generic_button_handler<E>(
        connection: Entity<Terminal>,
        focus_handle: FocusHandle,
        steal_focus: bool,
        f: impl Fn(&mut Terminal, &E, &mut Context<Terminal>),
    ) -> impl Fn(&E, &mut Window, &mut App) {
        move |event, window, cx| {
            if steal_focus {
                window.focus(&focus_handle, cx);
            } else if !focus_handle.is_focused(window) {
                return;
            }
            connection.update(cx, |terminal, cx| {
                f(terminal, event, cx);

                cx.notify();
            })
        }
    }

    pub(super) fn register_mouse_listeners(
        &mut self,
        mode: Modes,
        hitbox: &Hitbox,
        content_mode: &ContentMode,
        window: &mut Window,
    ) {
        let focus = self.focus.clone();
        let terminal = self.terminal.clone();
        let terminal_view = self.terminal_view.clone();

        self.interactivity.on_mouse_down(MouseButton::Left, {
            let terminal = terminal.clone();
            let focus = focus.clone();
            let terminal_view = terminal_view.clone();

            move |e, window, cx| {
                window.focus(&focus, cx);

                let scroll_top = terminal_view.read(cx).scroll_top;
                terminal.update(cx, |terminal, cx| {
                    let mut adjusted_event = e.clone();
                    if scroll_top > Pixels::ZERO {
                        adjusted_event.position.y += scroll_top;
                    }
                    terminal.mouse_down(&adjusted_event, cx);
                    cx.notify();
                })
            }
        });

        window.on_mouse_event({
            let terminal = self.terminal.clone();
            let hitbox = hitbox.clone();
            let focus = focus.clone();
            let terminal_view = terminal_view;
            move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }

                if e.pressed_button.is_some() && !cx.has_active_drag() && focus.is_focused(window) {
                    let hovered = hitbox.is_hovered(window);

                    let scroll_top = terminal_view.read(cx).scroll_top;
                    terminal.update(cx, |terminal, cx| {
                        if terminal.selection_started() || hovered {
                            let mut adjusted_event = e.clone();
                            if scroll_top > Pixels::ZERO {
                                adjusted_event.position.y += scroll_top;
                            }
                            terminal.mouse_drag(&adjusted_event, hitbox.bounds, cx);
                            cx.notify();
                        }
                    })
                }

                if hitbox.is_hovered(window) {
                    terminal.update(cx, |terminal, cx| {
                        terminal.mouse_move(e, cx);
                    })
                }
            }
        });

        self.interactivity.on_mouse_up(
            MouseButton::Left,
            TerminalElement::generic_button_handler(
                terminal.clone(),
                focus.clone(),
                false,
                move |terminal, e, cx| {
                    terminal.mouse_up(e, cx);
                },
            ),
        );
        self.interactivity.on_mouse_down(
            MouseButton::Middle,
            TerminalElement::generic_button_handler(
                terminal.clone(),
                focus.clone(),
                true,
                move |terminal, e, cx| {
                    terminal.mouse_down(e, cx);
                },
            ),
        );

        if content_mode.is_scrollable() {
            self.interactivity.on_scroll_wheel({
                let terminal_view = self.terminal_view.downgrade();
                move |e, window, cx| {
                    terminal_view
                        .update(cx, |terminal_view, cx| {
                            if matches!(terminal_view.mode, TerminalMode::Standalone)
                                || terminal_view.focus_handle.is_focused(window)
                            {
                                terminal_view.scroll_wheel(e, cx);
                                cx.notify();
                            }
                        })
                        .ok();
                }
            });
        }

        // Mouse mode handlers:
        // All mouse modes need the extra click handlers
        if mode.intersects(Modes::MOUSE_MODE) {
            self.interactivity.on_mouse_down(
                MouseButton::Right,
                TerminalElement::generic_button_handler(
                    terminal.clone(),
                    focus.clone(),
                    true,
                    move |terminal, e, cx| {
                        terminal.mouse_down(e, cx);
                    },
                ),
            );
            self.interactivity.on_mouse_up(
                MouseButton::Right,
                TerminalElement::generic_button_handler(
                    terminal.clone(),
                    focus.clone(),
                    false,
                    move |terminal, e, cx| {
                        terminal.mouse_up(e, cx);
                    },
                ),
            );
            self.interactivity.on_mouse_up(
                MouseButton::Middle,
                TerminalElement::generic_button_handler(
                    terminal,
                    focus,
                    false,
                    move |terminal, e, cx| {
                        terminal.mouse_up(e, cx);
                    },
                ),
            );
        }
    }
}
