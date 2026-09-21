use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    /// Renders the center pane group wrapped in a `Main` landmark so assistive
    /// technology recognizes the editor as the main region and can navigate to
    /// it. While a screen reader is active the wrapper is also the focus target
    /// for region navigation (it carries the landmark role and label), so
    /// focusing it announces "Editor" instead of falling back to the whole
    /// window. We only make it focusable in that case so it never adds a hitbox
    /// or intercepts mouse focus for other users.
    fn render_center(
        &self,
        render_cx: &PaneRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        div()
            .id("editor-region")
            .role(gpui::Role::Main)
            .aria_label("Editor")
            .when(window.is_a11y_active(), |this| {
                this.track_focus(&self.region_focus_handles.editor)
            })
            .size_full()
            .child(self.center.render(
                self.zoomed.as_ref(),
                self.maximized_pane.as_ref(),
                render_cx,
                window,
                cx,
            ))
    }
}
impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        static FIRST_PAINT: AtomicBool = AtomicBool::new(true);
        if FIRST_PAINT.swap(false, std::sync::atomic::Ordering::Relaxed) {
            log::info!("Rendered first frame");
        }

        let centered_layout = self.centered_layout
            && self.center.panes().len() == 1
            && self.active_item(cx).is_some();
        let render_padding = |size| {
            (size > 0.0).then(|| {
                div()
                    .h_full()
                    .w(relative(size))
                    .bg(cx.theme().colors().editor_background)
                    .border_color(cx.theme().colors().pane_group_border)
            })
        };
        let paddings = if centered_layout {
            let settings = WorkspaceSettings::get_global(cx).centered_layout;
            (
                render_padding(Self::adjust_padding(
                    settings.left_padding.map(|padding| padding.0),
                )),
                render_padding(Self::adjust_padding(
                    settings.right_padding.map(|padding| padding.0),
                )),
            )
        } else {
            (None, None)
        };
        let ui_font = theme_settings::setup_ui_font(window, cx);

        let theme = cx.theme().clone();
        let colors = theme.colors();
        let notification_entities = self
            .notifications
            .iter()
            .map(|(_, notification)| notification.entity_id())
            .collect::<Vec<_>>();
        let bottom_dock_layout = WorkspaceSettings::get_global(cx).bottom_dock_layout;

        let pane_render_context = PaneRenderContext {
            follower_states: &self.follower_states,
            active_call: self.active_call(),
            active_pane: &self.active_pane,
            app_state: &self.app_state,
            project: &self.project,
            workspace: &self.weak_self,
        };

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font(ui_font)
            .gap_0()
            .justify_start()
            .items_start()
            .text_color(colors.text)
            .overflow_hidden()
            // Expose the title bar as an ARIA toolbar so region navigation
            // (FocusNextPart) can reach the top bar's controls and assistive
            // technology announces it as a toolbar. The contained controls form
            // a tab group: region navigation lands on the first control (per
            // the ARIA toolbar pattern), Tab steps through them, and arrow keys
            // move between them once focus is inside.
            .when_some(self.titlebar_item.clone(), |this, item| {
                this.child(
                    div()
                        .id("titlebar-region")
                        .track_focus(&self.titlebar_focus_handle)
                        .tab_group()
                        .role(gpui::Role::Toolbar)
                        .aria_label("Title bar")
                        .on_key_down(cx.listener(
                            |workspace, event: &gpui::KeyDownEvent, window, cx| {
                                if event.keystroke.modifiers.modified() {
                                    return;
                                }
                                match event.keystroke.key.as_str() {
                                    "right" => {
                                        workspace.move_titlebar_item_focus(true, window, cx);
                                        cx.stop_propagation();
                                    }
                                    "left" => {
                                        workspace.move_titlebar_item_focus(false, window, cx);
                                        cx.stop_propagation();
                                    }
                                    _ => {}
                                }
                            },
                        ))
                        .w_full()
                        .child(item),
                )
            })
            .on_modifiers_changed(move |_, _, cx| {
                for &id in &notification_entities {
                    cx.notify(id);
                }
            })
            .child(
                div()
                    .size_full()
                    .relative()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("workspace")
                            .bg(colors.background)
                            .relative()
                            .flex_1()
                            .w_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .border_t_1()
                            .border_b_1()
                            .border_color(colors.border)
                            .child({
                                let this = cx.entity();
                                canvas(
                                    move |bounds, window, cx| {
                                        this.update(cx, |this, cx| {
                                            let bounds_changed = this.bounds != bounds;
                                            this.bounds = bounds;

                                            if bounds_changed {
                                                this.left_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.width,
                                                        window,
                                                        cx,
                                                    )
                                                });

                                                this.right_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.width,
                                                        window,
                                                        cx,
                                                    )
                                                });

                                                this.bottom_dock.update(cx, |dock, cx| {
                                                    dock.clamp_panel_size(
                                                        bounds.size.height,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            }
                                        })
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full()
                            })
                            .when(self.zoomed.is_none(), |this| {
                                this.on_drag_move(cx.listener(
                                    move |workspace, e: &DragMoveEvent<DraggedDock>, window, cx| {
                                        if workspace.previous_dock_drag_coordinates
                                            != Some(e.event.position)
                                        {
                                            workspace.previous_dock_drag_coordinates =
                                                Some(e.event.position);

                                            match e.drag(cx).0 {
                                                DockPosition::Left => {
                                                    workspace.resize_left_dock(
                                                        e.event.position.x
                                                            - workspace.bounds.left(),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                                DockPosition::Right => {
                                                    workspace.resize_right_dock(
                                                        workspace.bounds.right()
                                                            - e.event.position.x,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                                DockPosition::Bottom => {
                                                    workspace.resize_bottom_dock(
                                                        workspace.bounds.bottom()
                                                            - e.event.position.y,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            };
                                            workspace.serialize_workspace(window, cx);
                                        }
                                    },
                                ))
                            })
                            .child({
                                match bottom_dock_layout {
                                    BottomDockLayout::Full => div()
                                        .flex()
                                        .flex_col()
                                        .h_full()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .flex_1()
                                                .overflow_hidden()
                                                .children(self.render_dock(
                                                    DockPosition::Left,
                                                    &self.left_dock,
                                                    window,
                                                    cx,
                                                ))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .flex_1()
                                                        .overflow_hidden()
                                                        .child(
                                                            h_flex()
                                                                .flex_1()
                                                                .when_some(paddings.0, |this, p| {
                                                                    this.child(p.border_r_1())
                                                                })
                                                                .child(self.render_center(
                                                                    &pane_render_context,
                                                                    window,
                                                                    cx,
                                                                ))
                                                                .when_some(
                                                                    paddings.1,
                                                                    |this, p| {
                                                                        this.child(p.border_l_1())
                                                                    },
                                                                ),
                                                        ),
                                                )
                                                .children(self.render_dock(
                                                    DockPosition::Right,
                                                    &self.right_dock,
                                                    window,
                                                    cx,
                                                )),
                                        )
                                        .child(div().w_full().children(self.render_dock(
                                            DockPosition::Bottom,
                                            &self.bottom_dock,
                                            window,
                                            cx,
                                        ))),

                                    BottomDockLayout::LeftAligned => div()
                                        .flex()
                                        .flex_row()
                                        .h_full()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_1()
                                                .h_full()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .flex_1()
                                                        .children(self.render_dock(
                                                            DockPosition::Left,
                                                            &self.left_dock,
                                                            window,
                                                            cx,
                                                        ))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .flex_1()
                                                                .overflow_hidden()
                                                                .child(
                                                                    h_flex()
                                                                        .flex_1()
                                                                        .when_some(
                                                                            paddings.0,
                                                                            |this, p| {
                                                                                this.child(
                                                                                    p.border_r_1(),
                                                                                )
                                                                            },
                                                                        )
                                                                        .child(self.render_center(
                                                                            &pane_render_context,
                                                                            window,
                                                                            cx,
                                                                        ))
                                                                        .when_some(
                                                                            paddings.1,
                                                                            |this, p| {
                                                                                this.child(
                                                                                    p.border_l_1(),
                                                                                )
                                                                            },
                                                                        ),
                                                                ),
                                                        ),
                                                )
                                                .child(div().w_full().children(self.render_dock(
                                                    DockPosition::Bottom,
                                                    &self.bottom_dock,
                                                    window,
                                                    cx,
                                                ))),
                                        )
                                        .children(self.render_dock(
                                            DockPosition::Right,
                                            &self.right_dock,
                                            window,
                                            cx,
                                        )),
                                    BottomDockLayout::RightAligned => div()
                                        .flex()
                                        .flex_row()
                                        .h_full()
                                        .children(self.render_dock(
                                            DockPosition::Left,
                                            &self.left_dock,
                                            window,
                                            cx,
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_1()
                                                .h_full()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .flex_1()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .flex_1()
                                                                .overflow_hidden()
                                                                .child(
                                                                    h_flex()
                                                                        .flex_1()
                                                                        .when_some(
                                                                            paddings.0,
                                                                            |this, p| {
                                                                                this.child(
                                                                                    p.border_r_1(),
                                                                                )
                                                                            },
                                                                        )
                                                                        .child(self.render_center(
                                                                            &pane_render_context,
                                                                            window,
                                                                            cx,
                                                                        ))
                                                                        .when_some(
                                                                            paddings.1,
                                                                            |this, p| {
                                                                                this.child(
                                                                                    p.border_l_1(),
                                                                                )
                                                                            },
                                                                        ),
                                                                ),
                                                        )
                                                        .children(self.render_dock(
                                                            DockPosition::Right,
                                                            &self.right_dock,
                                                            window,
                                                            cx,
                                                        )),
                                                )
                                                .child(div().w_full().children(self.render_dock(
                                                    DockPosition::Bottom,
                                                    &self.bottom_dock,
                                                    window,
                                                    cx,
                                                ))),
                                        ),
                                    BottomDockLayout::Contained => div()
                                        .flex()
                                        .flex_row()
                                        .h_full()
                                        .children(self.render_dock(
                                            DockPosition::Left,
                                            &self.left_dock,
                                            window,
                                            cx,
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_1()
                                                .overflow_hidden()
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .when_some(paddings.0, |this, p| {
                                                            this.child(p.border_r_1())
                                                        })
                                                        .child(self.render_center(
                                                            &pane_render_context,
                                                            window,
                                                            cx,
                                                        ))
                                                        .when_some(paddings.1, |this, p| {
                                                            this.child(p.border_l_1())
                                                        }),
                                                )
                                                .children(self.render_dock(
                                                    DockPosition::Bottom,
                                                    &self.bottom_dock,
                                                    window,
                                                    cx,
                                                )),
                                        )
                                        .children(self.render_dock(
                                            DockPosition::Right,
                                            &self.right_dock,
                                            window,
                                            cx,
                                        )),
                                }
                            })
                            .children(self.zoomed.as_ref().and_then(|view| {
                                let zoomed_view = view.upgrade()?;
                                let div = div()
                                    .occlude()
                                    .absolute()
                                    .overflow_hidden()
                                    .border_color(colors.border)
                                    .bg(colors.background)
                                    .child(zoomed_view)
                                    .inset_0()
                                    .shadow_lg();

                                if !WorkspaceSettings::get_global(cx).zoomed_padding {
                                    return Some(div);
                                }

                                Some(match self.zoomed_position {
                                    Some(DockPosition::Left) => div.right_2().border_r_1(),
                                    Some(DockPosition::Right) => div.left_2().border_l_1(),
                                    Some(DockPosition::Bottom) => div.top_2().border_t_1(),
                                    None => div.top_2().bottom_2().left_2().right_2().border_1(),
                                })
                            }))
                            .children(self.render_notifications(window, cx)),
                    )
                    .when(self.status_bar_visible(cx), |parent| {
                        parent.child(self.status_bar.clone())
                    })
                    .child(self.toast_layer.clone()),
            )
    }
}

#[derive(Clone)]
struct DraggedDock(DockPosition);

impl Render for DraggedDock {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// Add client-side decorations (rounded corners, shadows, resize handling) when
/// appropriate.
pub fn client_side_decorations(
    element: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    const BORDER_SIZE: Pixels = px(1.0);
    let decorations = window.window_decorations();
    let is_resizable = window.is_resizable();
    let tiling = match decorations {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };

    match decorations {
        Decorations::Client { .. } => window.set_client_inset(theme::CLIENT_SIDE_DECORATION_SHADOW),
        Decorations::Server => window.set_client_inset(px(0.0)),
    }

    struct GlobalResizeEdge(ResizeEdge);
    impl Global for GlobalResizeEdge {}

    div()
        .id("window-backdrop")
        .bg(transparent_black())
        .map(|div| match decorations {
            Decorations::Server => div,
            Decorations::Client { .. } => div
                .rounded_client_corners(tiling)
                .when(!tiling.top, |div| {
                    div.pt(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.bottom, |div| {
                    div.pb(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.left, |div| {
                    div.pl(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.right, |div| {
                    div.pr(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(is_resizable, |div| {
                    div.on_mouse_move(move |e, window, cx| {
                        let size = window.window_bounds().get_bounds().size;
                        let pos = e.position;

                        let new_edge =
                            resize_edge(pos, theme::CLIENT_SIDE_DECORATION_SHADOW, size, tiling);

                        let edge = cx.try_global::<GlobalResizeEdge>();
                        if new_edge != edge.map(|edge| edge.0) {
                            window
                                .window_handle()
                                .update(cx, |workspace, _, cx| {
                                    cx.notify(workspace.entity_id());
                                })
                                .ok();
                        }
                    })
                    .on_mouse_down(MouseButton::Left, move |e, window, _| {
                        let size = window.window_bounds().get_bounds().size;
                        let pos = e.position;

                        let edge = match resize_edge(
                            pos,
                            theme::CLIENT_SIDE_DECORATION_SHADOW,
                            size,
                            tiling,
                        ) {
                            Some(value) => value,
                            None => return,
                        };

                        window.start_window_resize(edge);
                    })
                }),
        })
        .size_full()
        .child(
            div()
                .cursor(CursorStyle::Arrow)
                .map(|div| match decorations {
                    Decorations::Server => div,
                    Decorations::Client { .. } => div
                        .border_color(cx.theme().colors().border)
                        .rounded_client_corners(tiling)
                        .when(!tiling.top, |div| div.border_t(BORDER_SIZE))
                        .when(!tiling.bottom, |div| div.border_b(BORDER_SIZE))
                        .when(!tiling.left, |div| div.border_l(BORDER_SIZE))
                        .when(!tiling.right, |div| div.border_r(BORDER_SIZE))
                        .when(!tiling.is_tiled(), |div| {
                            div.shadow(vec![
                                gpui::BoxShadow::new(
                                    px(0.),
                                    px(0.),
                                    Hsla {
                                        h: 0.,
                                        s: 0.,
                                        l: 0.,
                                        a: 0.4,
                                    },
                                )
                                .blur_radius(theme::CLIENT_SIDE_DECORATION_SHADOW / 2.),
                            ])
                        }),
                })
                .on_mouse_move(|_e, _, cx| {
                    cx.stop_propagation();
                })
                .size_full()
                .child(element),
        )
        .map(|div| match decorations {
            Decorations::Server => div,
            Decorations::Client { tiling, .. } if is_resizable => div.child(
                canvas(
                    |_bounds, window, _| {
                        window.insert_hitbox(
                            Bounds::new(
                                point(px(0.0), px(0.0)),
                                window.window_bounds().get_bounds().size,
                            ),
                            HitboxBehavior::Normal,
                        )
                    },
                    move |_bounds, hitbox, window, cx| {
                        let mouse = window.mouse_position();
                        let size = window.window_bounds().get_bounds().size;
                        let Some(edge) =
                            resize_edge(mouse, theme::CLIENT_SIDE_DECORATION_SHADOW, size, tiling)
                        else {
                            return;
                        };
                        cx.set_global(GlobalResizeEdge(edge));
                        window.set_cursor_style(
                            match edge {
                                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                                ResizeEdge::Left | ResizeEdge::Right => {
                                    CursorStyle::ResizeLeftRight
                                }
                                ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                    CursorStyle::ResizeUpLeftDownRight
                                }
                                ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                    CursorStyle::ResizeUpRightDownLeft
                                }
                            },
                            &hitbox,
                        );
                    },
                )
                .size_full()
                .absolute(),
            ),
            Decorations::Client { .. } => div,
        })
}

fn resize_edge(
    pos: Point<Pixels>,
    shadow_size: Pixels,
    window_size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let bounds = Bounds::new(Point::default(), window_size).inset(shadow_size * 1.5);
    if bounds.contains(&pos) {
        return None;
    }

    let corner_size = size(shadow_size * 1.5, shadow_size * 1.5);
    let top_left_bounds = Bounds::new(Point::new(px(0.), px(0.)), corner_size);
    if !tiling.top && top_left_bounds.contains(&pos) {
        return Some(ResizeEdge::TopLeft);
    }

    let top_right_bounds = Bounds::new(
        Point::new(window_size.width - corner_size.width, px(0.)),
        corner_size,
    );
    if !tiling.top && top_right_bounds.contains(&pos) {
        return Some(ResizeEdge::TopRight);
    }

    let bottom_left_bounds = Bounds::new(
        Point::new(px(0.), window_size.height - corner_size.height),
        corner_size,
    );
    if !tiling.bottom && bottom_left_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomLeft);
    }

    let bottom_right_bounds = Bounds::new(
        Point::new(
            window_size.width - corner_size.width,
            window_size.height - corner_size.height,
        ),
        corner_size,
    );
    if !tiling.bottom && bottom_right_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomRight);
    }

    if !tiling.top && pos.y < shadow_size {
        Some(ResizeEdge::Top)
    } else if !tiling.bottom && pos.y > window_size.height - shadow_size {
        Some(ResizeEdge::Bottom)
    } else if !tiling.left && pos.x < shadow_size {
        Some(ResizeEdge::Left)
    } else if !tiling.right && pos.x > window_size.width - shadow_size {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}
