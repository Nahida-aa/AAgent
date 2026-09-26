use super::*;

use anyhow::anyhow;
use gpui::{
    Anchor, App, Context, Entity, IntoElement, MouseButton, ParentElement, Render, SharedString,
    Styled, Subscription, Window,
};
use settings::SettingsStore;
use ui::{
    ContextMenu, CountBadge, Divider, DividerColor, IconButton, IconPosition, IconSize, Tooltip,
    prelude::*, right_click_menu,
};
use util::ResultExt as _;

use crate::status_bar::{HideStatusItem, StatusItemView};

use super::entity::Dock;
use super::position::DockPosition;

pub struct PanelButtons {
    pub(super) dock: Entity<Dock>,
    _settings_subscription: Subscription,
}

impl PanelButtons {
    pub fn new(dock: Entity<Dock>, cx: &mut Context<Self>) -> Self {
        cx.observe(&dock, |_, _, cx| cx.notify()).detach();
        let settings_subscription = cx.observe_global::<SettingsStore>(|_, cx| cx.notify());
        Self {
            dock,
            _settings_subscription: settings_subscription,
        }
    }
}

impl Render for PanelButtons {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dock = self.dock.read(cx);
        let active_index = dock.active_panel_index;
        let is_open = dock.is_open;
        let dock_position = dock.position;

        let (menu_anchor, menu_attach) = match dock.position {
            DockPosition::Left => (Anchor::BottomLeft, Anchor::TopLeft),
            DockPosition::Bottom | DockPosition::Right => (Anchor::BottomRight, Anchor::TopRight),
        };

        let dock_entity = self.dock.clone();
        let workspace = dock.workspace.clone();
        let mut buttons: Vec<_> = dock
            .panel_entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let icon = entry.panel.icon(window, cx)?;
                let icon_tooltip = entry
                    .panel
                    .icon_tooltip(window, cx)
                    .ok_or_else(|| {
                        anyhow::anyhow!("can't render a panel button without an icon tooltip")
                    })
                    .log_err()?;
                let name = entry.panel.persistent_name();
                let panel = entry.panel.clone();
                let supports_flexible = panel.supports_flexible_size(cx);
                let currently_flexible = panel.has_flexible_size(window, cx);
                let dock_for_menu = dock_entity.clone();
                let workspace_for_menu = workspace.clone();

                let is_active_button = Some(i) == active_index && is_open;
                let (action, tooltip) = if is_active_button {
                    let action = dock.toggle_action();

                    let tooltip: SharedString =
                        format!("Close {} Dock", dock.position.label()).into();

                    (action, tooltip)
                } else {
                    let action = entry.panel.toggle_action(window, cx);

                    (action, icon_tooltip.into())
                };

                let focus_handle = dock.focus_handle(cx);
                let icon_label = entry.panel.icon_label(window, cx);

                Some(
                    right_click_menu(name)
                        .menu(move |window, cx| {
                            const POSITIONS: [DockPosition; 3] = [
                                DockPosition::Left,
                                DockPosition::Right,
                                DockPosition::Bottom,
                            ];

                            let panel_hide = panel.hide_button_setting(cx);
                            ContextMenu::build(window, cx, |mut menu, _, cx| {
                                let mut has_position_entries = false;
                                for position in POSITIONS {
                                    if panel.position_is_valid(position, cx) {
                                        let is_current = position == dock_position;
                                        let panel = panel.clone();
                                        menu = menu.toggleable_entry(
                                            format!("Dock {}", position.label()),
                                            is_current,
                                            IconPosition::Start,
                                            None,
                                            move |window, cx| {
                                                if !is_current {
                                                    panel.set_position(position, window, cx);
                                                }
                                            },
                                        );
                                        has_position_entries = true;
                                    }
                                }
                                if supports_flexible {
                                    if has_position_entries {
                                        menu = menu.separator();
                                    }
                                    let panel_for_flex = panel.clone();
                                    let dock_for_flex = dock_for_menu.clone();
                                    let workspace_for_flex = workspace_for_menu.clone();
                                    menu = menu.toggleable_entry(
                                        "Flex Width",
                                        currently_flexible,
                                        IconPosition::Start,
                                        None,
                                        move |window, cx| {
                                            if !currently_flexible {
                                                if let Some(ws) = workspace_for_flex.upgrade() {
                                                    ws.update(cx, |workspace, cx| {
                                                        workspace.toggle_dock_panel_flexible_size(
                                                            &dock_for_flex,
                                                            panel_for_flex.as_ref(),
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            }
                                        },
                                    );
                                    let panel_for_fixed = panel.clone();
                                    let dock_for_fixed = dock_for_menu.clone();
                                    let workspace_for_fixed = workspace_for_menu.clone();
                                    menu = menu.toggleable_entry(
                                        "Fixed Width",
                                        !currently_flexible,
                                        IconPosition::Start,
                                        None,
                                        move |window, cx| {
                                            if currently_flexible {
                                                if let Some(ws) = workspace_for_fixed.upgrade() {
                                                    ws.update(cx, |workspace, cx| {
                                                        workspace.toggle_dock_panel_flexible_size(
                                                            &dock_for_fixed,
                                                            panel_for_fixed.as_ref(),
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            }
                                        },
                                    );
                                }
                                if let Some(hide) = panel_hide {
                                    menu = crate::status_bar::add_hide_button_entry(
                                        menu.separator(),
                                        hide,
                                    );
                                }
                                menu
                            })
                        })
                        .anchor(menu_anchor)
                        .attach(menu_attach)
                        .trigger(move |is_active, _window, _cx| {
                            // Include active state in element ID to invalidate the cached
                            // tooltip when panel state changes (e.g., via keyboard shortcut)
                            let button = IconButton::new((name, is_active_button as u64), icon)
                                .icon_size(IconSize::Small)
                                .toggle_state(is_active_button)
                                .tab_index(0isize)
                                .aria_label(icon_tooltip)
                                .on_click({
                                    let action = action.boxed_clone();
                                    move |_, window, cx| {
                                        window.focus(&focus_handle, cx);
                                        window.dispatch_action(action.boxed_clone(), cx)
                                    }
                                })
                                .when(!is_active, |this| {
                                    this.tooltip(move |_window, cx| {
                                        Tooltip::for_action(tooltip.clone(), &*action, cx)
                                    })
                                });

                            div().relative().child(button).when_some(
                                icon_label
                                    .clone()
                                    .filter(|_| !is_active_button)
                                    .and_then(|label| label.parse::<usize>().ok()),
                                |this, count| this.child(CountBadge::new(count)),
                            )
                        }),
                )
            })
            .collect();

        if dock_position == DockPosition::Right {
            buttons.reverse();
        }

        let has_buttons = !buttons.is_empty();

        h_flex()
            .gap_1()
            .when(
                has_buttons
                    && (dock.position == DockPosition::Bottom
                        || dock.position == DockPosition::Right),
                |this| this.child(Divider::vertical().color(DividerColor::Border)),
            )
            .children(buttons)
            .when(has_buttons && dock.position == DockPosition::Left, |this| {
                this.child(Divider::vertical().color(DividerColor::Border))
            })
    }
}

impl StatusItemView for PanelButtons {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn crate::ItemHandle>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Nothing to do, panel buttons don't depend on the active center item
    }

    fn hide_setting(&self, _: &App) -> Option<HideStatusItem> {
        // Panel buttons are hidden on a per-panel basis through each panel
        // button's own context menu.
        None
    }
}
