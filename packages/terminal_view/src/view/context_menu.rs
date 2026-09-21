use gpui::{
    App, Context, DismissEvent, Entity, Focusable, Pixels, Point as GpuiPoint, Subscription, Window,
};
use settings::Settings;
use terminal::terminal_settings::TerminalSettings;
use terminal::{Clear, Copy, Paste, PasteText};
use ui::ContextMenu;
use workspace::{CloseActiveItem, NewCenterTerminal, NewTerminal};
use zed_actions::{agent::AddSelectionToThread, assistant::InlineAssist};

use crate::terminal_panel::TerminalPanel;

use super::TerminalView;
use super::mode::TerminalMode;

impl TerminalView {
    pub(super) fn shows_workspace_actions(&self) -> bool {
        self.show_workspace_actions
            .unwrap_or_else(|| !matches!(self.mode, TerminalMode::Embedded { .. }))
    }

    pub fn set_show_workspace_actions(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_workspace_actions = Some(show);
        cx.notify();
    }

    pub(super) fn deploy_context_menu(
        &mut self,
        position: GpuiPoint<Pixels>,
        has_selection: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let assistant_enabled = self
            .workspace
            .upgrade()
            .and_then(|workspace| workspace.read(cx).panel::<TerminalPanel>(cx))
            .is_some_and(|terminal_panel| terminal_panel.read(cx).assistant_enabled());
        let context_menu = ContextMenu::build(window, cx, |menu, _, _| {
            menu.context(self.focus_handle.clone())
                .when(self.shows_workspace_actions(), |menu| {
                    menu.action("New Terminal", Box::new(NewTerminal::default()))
                        .action(
                            "New Center Terminal",
                            Box::new(NewCenterTerminal::default()),
                        )
                        .separator()
                })
                .action("Copy", Box::new(Copy))
                .when(
                    !matches!(self.mode, TerminalMode::Embedded { .. }),
                    |menu| {
                        menu.action("Paste", Box::new(Paste))
                            .action("Paste Text", Box::new(PasteText))
                    },
                )
                .action("Select All", Box::new(editor::actions::SelectAll))
                .when(
                    !matches!(self.mode, TerminalMode::Embedded { .. }),
                    |menu| menu.action("Clear", Box::new(Clear)),
                )
                .when(
                    assistant_enabled && !matches!(self.mode, TerminalMode::Embedded { .. }),
                    |menu| {
                        menu.separator()
                            .action("Inline Assist", Box::new(InlineAssist::default()))
                            .when(has_selection && self.shows_workspace_actions(), |menu| {
                                menu.action("Add to Agent Thread", Box::new(AddSelectionToThread))
                            })
                    },
                )
                .when(self.shows_workspace_actions(), |menu| {
                    menu.separator().action(
                        "Close Terminal Tab",
                        Box::new(CloseActiveItem {
                            save_intent: None,
                            close_pinned: true,
                        }),
                    )
                })
        });

        window.focus(&context_menu.focus_handle(cx), cx);
        let subscription = cx.subscribe_in(
            &context_menu,
            window,
            |this, _, _: &DismissEvent, window, cx| {
                if this.context_menu.as_ref().is_some_and(|context_menu| {
                    context_menu.0.focus_handle(cx).contains_focused(window, cx)
                }) {
                    cx.focus_self(window);
                }
                this.context_menu.take();
                cx.notify();
            },
        );

        self.context_menu = Some((context_menu, position, subscription));
    }
}
