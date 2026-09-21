use gpui::{
    Anchor, App, Context, Focusable, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window,
};
use terminal::Terminal;
use ui::prelude::*;
use ui::{ContextMenu, IconButton, IconName, IconSize, PopoverMenu, Tooltip};
use workspace::{Pane, SplitDown, SplitLeft, SplitRight, SplitUp, ToggleZoom, Workspace};
use zed_actions::assistant::InlineAssist;

use super::TerminalPanel;

impl TerminalPanel {
    pub fn set_assistant_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.assistant_enabled = enabled;
        for pane in self.center.panes() {
            self.apply_tab_bar_buttons(pane, cx);
        }
    }

    pub(crate) fn apply_tab_bar_buttons(
        &self,
        terminal_pane: &Entity<Pane>,
        cx: &mut Context<Self>,
    ) {
        let assistant_enabled = self.assistant_enabled;
        terminal_pane.update(cx, |pane, cx| {
            pane.set_render_tab_bar_buttons(cx, move |pane, window, cx| {
                let split_context = pane
                    .active_item()
                    .and_then(|item| item.downcast::<crate::TerminalView>())
                    .map(|terminal_view| terminal_view.read(cx).focus_handle.clone());
                let has_focused_rename_editor = pane
                    .active_item()
                    .and_then(|item| item.downcast::<crate::TerminalView>())
                    .is_some_and(|view| view.read(cx).rename_editor_is_focused(window, cx));
                if !pane.has_focus(window, cx)
                    && !pane.context_menu_focused(window, cx)
                    && !has_focused_rename_editor
                {
                    return (None, None);
                }
                let focus_handle = pane.focus_handle(cx);
                let right_children = h_flex()
                    .gap(DynamicSpacing::Base02.rems(cx))
                    .child(
                        PopoverMenu::new("terminal-tab-bar-popover-menu")
                            .trigger_with_tooltip(
                                IconButton::new("plus", IconName::Plus).icon_size(IconSize::Small),
                                Tooltip::text("New…"),
                            )
                            .anchor(Anchor::TopRight)
                            .with_handle(pane.new_item_context_menu_handle.clone())
                            .menu(move |window, cx| {
                                let focus_handle = focus_handle.clone();
                                let menu = ContextMenu::build(window, cx, |menu, _, _| {
                                    menu.context(focus_handle.clone())
                                        .action(
                                            "New Terminal",
                                            workspace::NewTerminal::default().boxed_clone(),
                                        )
                                        .action(
                                            "Spawn Task",
                                            zed_actions::Spawn::modal().boxed_clone(),
                                        )
                                });

                                Some(menu)
                            }),
                    )
                    .when(assistant_enabled, |this| {
                        this.when_some(split_context.clone(), |this, focus_handle| {
                            this.child(InlineAssistTabBarButton { focus_handle })
                        })
                    })
                    .child(
                        PopoverMenu::new("terminal-pane-tab-bar-split")
                            .trigger_with_tooltip(
                                IconButton::new("terminal-pane-split", IconName::Split)
                                    .icon_size(IconSize::Small),
                                Tooltip::text("Split Pane"),
                            )
                            .anchor(Anchor::TopRight)
                            .with_handle(pane.split_item_context_menu_handle.clone())
                            .menu({
                                move |window, cx| {
                                    ContextMenu::build(window, cx, |menu, _, _| {
                                        menu.when_some(
                                            split_context.clone(),
                                            |menu, split_context| menu.context(split_context),
                                        )
                                        .action("Split Right", SplitRight::default().boxed_clone())
                                        .action("Split Left", SplitLeft::default().boxed_clone())
                                        .action("Split Up", SplitUp::default().boxed_clone())
                                        .action("Split Down", SplitDown::default().boxed_clone())
                                    })
                                    .into()
                                }
                            }),
                    )
                    .child({
                        let zoomed = pane.is_zoomed();
                        IconButton::new("toggle_zoom", IconName::Maximize)
                            .icon_size(IconSize::Small)
                            .toggle_state(zoomed)
                            .selected_icon(IconName::Minimize)
                            .on_click(cx.listener(|pane, _, window, cx| {
                                pane.toggle_zoom(&ToggleZoom, window, cx);
                            }))
                            .tooltip(move |_window, cx| {
                                Tooltip::for_action(
                                    if zoomed { "Zoom Out" } else { "Zoom In" },
                                    &ToggleZoom,
                                    cx,
                                )
                            })
                    })
                    .into_any_element()
                    .into();
                (None, right_children)
            });
        });
    }
}

#[derive(IntoElement)]
pub(super) struct InlineAssistTabBarButton {
    pub(super) focus_handle: gpui::FocusHandle,
}

impl RenderOnce for InlineAssistTabBarButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let focus_handle = self.focus_handle;
        IconButton::new("terminal_inline_assistant", IconName::ZedAssistant)
            .icon_size(IconSize::Small)
            .on_click({
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    focus_handle.dispatch_action(&InlineAssist::default(), window, cx);
                }
            })
            .tooltip(move |_window, cx| {
                Tooltip::for_action_in("Inline Assist", &InlineAssist::default(), &focus_handle, cx)
            })
    }
}
