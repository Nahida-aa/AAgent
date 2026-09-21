use editor::{Editor, actions::SelectAll};
use gpui::{Context, Entity, Subscription, Window};

use super::TerminalView;
use super::actions::RenameTerminal;

impl TerminalView {
    pub fn custom_title(&self) -> Option<&str> { self.custom_title.as_deref() }

    pub fn set_custom_title(&mut self, label: Option<String>, cx: &mut Context<Self>) {
        let label = label.filter(|l| !l.trim().is_empty());
        if self.custom_title != label {
            self.custom_title = label;
            self.needs_serialize = true;
            cx.emit(workspace::item::ItemEvent::UpdateTab);
            cx.notify();
        }
    }

    pub(super) fn mark_needs_serialize(&mut self, cx: &mut Context<Self>) {
        self.needs_serialize = true;
        cx.emit(workspace::item::ItemEvent::UpdateTab);
    }

    pub fn is_renaming(&self) -> bool { self.rename_editor.is_some() }

    pub fn rename_editor_is_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        use gpui::Focusable;
        self.rename_editor
            .as_ref()
            .is_some_and(|editor| editor.focus_handle(cx).is_focused(window))
    }

    pub(super) fn finish_renaming(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui::Focusable;
        let Some(editor) = self.rename_editor.take() else {
            return;
        };
        self.rename_editor_subscription = None;
        if save {
            let new_label = editor.read(cx).text(cx).trim().to_string();
            let label = if new_label.is_empty() {
                None
            } else {
                let terminal_title = self.terminal.read(cx).title(true);
                if new_label == terminal_title {
                    None
                } else {
                    Some(new_label)
                }
            };
            self.set_custom_title(label, cx);
        }
        cx.notify();
        self.focus_handle.focus(window, cx);
    }

    pub(super) fn rename_terminal(
        &mut self,
        _: &RenameTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui::Focusable;
        if self.terminal.read(cx).task().is_some() {
            return;
        }

        let current_label = self
            .custom_title
            .clone()
            .unwrap_or_else(|| self.terminal.read(cx).title(true));

        let rename_editor = cx.new(|cx| Editor::single_line(window, cx));
        let rename_editor_subscription = cx.subscribe_in(&rename_editor, window, {
            let rename_editor = rename_editor.clone();
            move |_this, _, event, window, cx| {
                if let editor::EditorEvent::Blurred = event {
                    let rename_editor = rename_editor.clone();
                    cx.defer_in(window, move |this, window, cx| {
                        let still_current = this
                            .rename_editor
                            .as_ref()
                            .is_some_and(|current| current == &rename_editor);
                        if still_current && !rename_editor.focus_handle(cx).is_focused(window) {
                            this.finish_renaming(false, window, cx);
                        }
                    });
                }
            }
        });

        self.rename_editor = Some(rename_editor.clone());
        self.rename_editor_subscription = Some(rename_editor_subscription);

        rename_editor.update(cx, |editor, cx| {
            editor.set_text(current_label, window, cx);
            editor.select_all(&SelectAll, window, cx);
            editor.focus_handle(cx).focus(window, cx);
        });
        cx.notify();
    }
}
