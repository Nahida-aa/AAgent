use gpui::{
    Anchor, App, Context, IntoElement, ParentElement, Render, Role, SharedString, Styled, Window,
};
use theme::CLIENT_SIDE_DECORATION_ROUNDING;
use ui::{ContextMenu, Divider, IconPosition, Indicator, Tooltip, prelude::*, right_click_menu};

use crate::sidebar_side_context_menu;
use crate::{MultiWorkspace, SidebarSide, ToggleWorkspaceSidebar};

use super::item::{HideStatusItem, StatusItemViewHandle};
use super::sidebar_status::SidebarStatus;
use super::status_bar::StatusBar;

impl Render for StatusBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = SidebarStatus::query(&self.multi_workspace, cx);

        h_flex()
            .id("status-bar")
            .track_focus(&self.focus_handle)
            .key_context("StatusBar")
            // Expose the status bar as an ARIA toolbar so assistive technology
            // announces it as a toolbar and region navigation can reach its
            // controls. The controls inside form a tab group: region navigation
            // lands on the first control (per the ARIA toolbar pattern), Tab
            // steps through them, and arrow keys move between them once focus is
            // inside.
            .role(Role::Toolbar)
            .aria_label("Status bar")
            .tab_group()
            .on_key_down(
                cx.listener(|status_bar, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    match event.keystroke.key.as_str() {
                        "right" => {
                            status_bar.move_item_focus(true, window, cx);
                            cx.stop_propagation();
                        }
                        "left" => {
                            status_bar.move_item_focus(false, window, cx);
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                }),
            )
            .w_full()
            .justify_between()
            .gap(DynamicSpacing::Base08.rems(cx))
            .p(DynamicSpacing::Base04.rems(cx))
            .bg(cx.theme().colors().status_bar_background)
            .map(|el| match window.window_decorations() {
                Decorations::Server => el,
                Decorations::Client { tiling, .. } => el
                    .when(
                        !(tiling.bottom || tiling.right)
                            && !(sidebar.open && sidebar.side == SidebarSide::Right),
                        |el| el.rounded_br(CLIENT_SIDE_DECORATION_ROUNDING),
                    )
                    .when(
                        !(tiling.bottom || tiling.left)
                            && !(sidebar.open && sidebar.side == SidebarSide::Left),
                        |el| el.rounded_bl(CLIENT_SIDE_DECORATION_ROUNDING),
                    )
                    // This border is to avoid a transparent gap in the rounded corners
                    .mb(px(-1.))
                    .mt({
                        #[cfg(target_os = "linux")]
                        let needs_gap_fix = {
                            // Running on Wayland and using some scaling levels other than 100% causes a
                            // 1px gap above the status bar; adding a margin avoids this.
                            gpui::guess_compositor() == "Wayland" && window.scale_factor() != 1.0
                        };
                        #[cfg(not(target_os = "linux"))]
                        let needs_gap_fix = false;
                        if needs_gap_fix { px(-1.) } else { px(0.) }
                    })
                    .border_b(px(1.0))
                    .border_color(cx.theme().colors().status_bar_background),
            })
            .child(self.render_left_tools(&sidebar, cx))
            .child(self.render_right_tools(&sidebar, cx))
    }
}

impl StatusBar {
    pub(crate) fn render_left_tools(
        &self,
        sidebar: &SidebarStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_1()
            .min_w_0()
            .overflow_x_hidden()
            .when(
                sidebar.show_toggle && !sidebar.open && sidebar.side == SidebarSide::Left,
                |this| this.child(self.render_sidebar_toggle(sidebar, cx)),
            )
            .children(self.left_items.iter().enumerate().map(|(index, item)| {
                render_hideable_item("status-bar-left", index, item.as_ref(), cx)
            }))
    }

    pub(crate) fn render_right_tools(
        &self,
        sidebar: &SidebarStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .flex_shrink_0()
            .gap_1()
            .overflow_x_hidden()
            .children(
                self.right_items
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(index, item)| {
                        render_hideable_item("status-bar-right", index, item.as_ref(), cx)
                    }),
            )
            .when(
                sidebar.show_toggle && !sidebar.open && sidebar.side == SidebarSide::Right,
                |this| this.child(self.render_sidebar_toggle(sidebar, cx)),
            )
    }

    pub(crate) fn render_sidebar_toggle(
        &self,
        sidebar: &SidebarStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on_right = sidebar.side == SidebarSide::Right;
        let has_notifications = sidebar.has_notifications;
        let indicator_border = cx.theme().colors().status_bar_background;

        let toggle = sidebar_side_context_menu("sidebar-status-toggle-menu", cx)
            .anchor(if on_right {
                Anchor::BottomRight
            } else {
                Anchor::BottomLeft
            })
            .attach(if on_right {
                Anchor::TopRight
            } else {
                Anchor::TopLeft
            })
            .trigger(move |_is_active, _window, _cx| {
                IconButton::new(
                    "toggle-workspace-sidebar",
                    if on_right {
                        IconName::ThreadsSidebarRightClosed
                    } else {
                        IconName::ThreadsSidebarLeftClosed
                    },
                )
                .icon_size(IconSize::Small)
                .tab_index(0isize)
                .aria_label("Open threads sidebar")
                .when(has_notifications, |this| {
                    this.indicator(Indicator::dot().color(Color::Accent))
                        .indicator_border_color(Some(indicator_border))
                })
                .tooltip(move |_, cx| {
                    Tooltip::for_action("Open Threads Sidebar", &ToggleWorkspaceSidebar, cx)
                })
                .on_click(move |_, window, cx| {
                    if let Some(multi_workspace) = window.root::<MultiWorkspace>().flatten() {
                        multi_workspace.update(cx, |multi_workspace, cx| {
                            multi_workspace.toggle_sidebar(window, cx);
                        });
                    }
                })
            });

        h_flex()
            .gap_0p5()
            .when(on_right, |this| {
                this.child(Divider::vertical().color(ui::DividerColor::Border))
            })
            .child(toggle)
            .when(!on_right, |this| {
                this.child(Divider::vertical().color(ui::DividerColor::Border))
            })
    }
}

fn render_hideable_item(
    side: &'static str,
    index: usize,
    item: &dyn StatusItemViewHandle,
    cx: &App,
) -> impl IntoElement {
    let view = item.to_any();
    let Some(hide) = item.hide_setting(cx) else {
        return view.into_any_element();
    };

    let menu_id: SharedString = format!("{side}-item-menu-{index}").into();
    right_click_menu(menu_id)
        .trigger(move |_is_active, _window, _cx| view)
        .menu(move |window, cx| {
            let hide = hide.clone();
            ContextMenu::build(window, cx, move |menu, _window, _cx| {
                add_hide_button_entry(menu, hide)
            })
        })
        .into_any_element()
}

/// Appends a "Hide Button" entry aligned with surrounding toggleable entries.
pub fn add_hide_button_entry(menu: ContextMenu, hide: HideStatusItem) -> ContextMenu {
    menu.toggleable_entry(
        "Hide Button",
        false,
        IconPosition::Start,
        None,
        move |_window, cx| hide.apply(cx),
    )
}
