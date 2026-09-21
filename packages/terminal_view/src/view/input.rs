use super::TerminalView;
use gpui::{Context, KeyDownEvent, Keystroke, Window};
use terminal::Terminal;
use terminal::terminal_settings::TerminalSettings;

impl TerminalView {
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_bell(cx);
        self.pause_cursor_blinking(window, cx);

        if event.prefer_character_input
            && event.keystroke.key_char.is_some()
            && !self.terminal.read(cx).vi_mode_enabled()
        {
            return;
        }

        if self.process_keystroke(&event.keystroke, cx) {
            cx.stop_propagation();
        }
    }
    /// Attempts to process a keystroke in the terminal. Returns true if handled.
    ///
    /// In vi mode, explicitly triggers a re-render because vi navigation (like j/k)
    /// updates the cursor locally without sending data to the shell, so there's no
    /// shell output to automatically trigger a re-render.
    fn process_keystroke(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let (handled, vi_mode_enabled) = self.terminal.update(cx, |term, cx| {
            (
                term.try_keystroke(keystroke, TerminalSettings::get_global(cx).option_as_meta),
                term.vi_mode_enabled(),
            )
        });

        if handled && vi_mode_enabled {
            cx.notify();
        }

        handled
    }
    pub(super) fn send_text(&mut self, text: &SendText, _: &mut Window, cx: &mut Context<Self>) {
        self.clear_bell(cx);
        self.blink_manager.update(cx, BlinkManager::pause_blinking);
        self.terminal.update(cx, |term, _| {
            term.input(text.0.to_string().into_bytes());
        });
    }
    pub(super) fn send_keystroke(
        &mut self,
        text: &SendKeystroke,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(keystroke) = Keystroke::parse(&text.0).log_err() {
            self.clear_bell(cx);
            self.blink_manager.update(cx, BlinkManager::pause_blinking);
            self.process_keystroke(&keystroke, cx);
        }
    }

    ///Attempt to paste the clipboard into the terminal
    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |term, _| term.copy(None));
        cx.notify();
    }

    /// Specific handler for the [`editor::actions::Copy`] action in order for
    /// the `Edit > Copy` menu item to not be disabled, as the app expects a
    /// handler for this action in order to enable/disable the menu item.
    pub(super) fn editor_copy(
        &mut self,
        _: &editor::actions::Copy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copy(&Copy, window, cx);
    }

    ///Attempt to paste the clipboard into the terminal
    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };

        match clipboard.entries().first() {
            Some(ClipboardEntry::Image(image)) if !image.bytes.is_empty() => {
                self.forward_ctrl_v(cx);
            }
            Some(ClipboardEntry::ExternalPaths(paths)) => {
                self.add_paths_to_terminal(paths.paths(), window, cx);
            }
            _ => {
                if let Some(text) = clipboard.text() {
                    self.terminal
                        .update(cx, |terminal, _cx| terminal.paste(&text));
                }
            }
        }
    }

    /// Specific handler for the [`editor::actions::Paste`] action in order for
    /// the `Edit > Paste` menu item to not be disabled, as the app expects a
    /// handler for this action in order to enable/disable the menu item.
    pub(super) fn editor_paste(
        &mut self,
        _: &editor::actions::Paste,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste(&Paste, window, cx);
    }

    ///Attempt to paste the clipboard text into the terminal
    pub(super) fn paste_text(&mut self, _: &PasteText, _: &mut Window, cx: &mut Context<Self>) {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };

        if let Some(text) = clipboard.text() {
            self.terminal
                .update(cx, |terminal, _cx| terminal.paste(&text));
        }
    }

    /// Emits a raw Ctrl+V so TUI agents can read the OS clipboard directly
    /// and attach images using their native workflows.
    fn forward_ctrl_v(&self, cx: &mut Context<Self>) {
        self.terminal.update(cx, |term, _| {
            term.input(vec![0x16]);
        });
    }

    pub fn add_paths_to_terminal(&self, paths: &[PathBuf], window: &mut Window, cx: &mut App) {
        let mut text = paths
            .iter()
            .filter_map(|path| Some(format!(" {}", shlex::try_quote(path.to_str()?).ok()?)))
            .collect::<String>();
        text.push(' ');
        window.focus(&self.focus_handle(cx), cx);
        self.terminal.update(cx, |terminal, _| {
            terminal.paste(&text);
        });
    }

    pub(super) fn dispatch_context(&self, cx: &App) -> KeyContext {
        let mut dispatch_context = KeyContext::new_with_defaults();
        dispatch_context.add("Terminal");

        if self.terminal.read(cx).vi_mode_enabled() {
            dispatch_context.add("vi_mode");
        }

        let mode = self.terminal.read(cx).last_content.mode;
        dispatch_context.set(
            "screen",
            if mode.contains(Modes::ALT_SCREEN) {
                "alt"
            } else {
                "normal"
            },
        );

        if mode.contains(Modes::APP_CURSOR) {
            dispatch_context.add("DECCKM");
        }
        if mode.contains(Modes::APP_KEYPAD) {
            dispatch_context.add("DECPAM");
        } else {
            dispatch_context.add("DECPNM");
        }
        if mode.contains(Modes::SHOW_CURSOR) {
            dispatch_context.add("DECTCEM");
        }
        if mode.contains(Modes::LINE_WRAP) {
            dispatch_context.add("DECAWM");
        }
        if mode.contains(Modes::ORIGIN) {
            dispatch_context.add("DECOM");
        }
        if mode.contains(Modes::INSERT) {
            dispatch_context.add("IRM");
        }
        //LNM is apparently the name for this. https://vt100.net/docs/vt510-rm/LNM.html
        if mode.contains(Modes::LINE_FEED_NEW_LINE) {
            dispatch_context.add("LNM");
        }
        if mode.contains(Modes::FOCUS_IN_OUT) {
            dispatch_context.add("report_focus");
        }
        if mode.contains(Modes::ALTERNATE_SCROLL) {
            dispatch_context.add("alternate_scroll");
        }
        if mode.contains(Modes::BRACKETED_PASTE) {
            dispatch_context.add("bracketed_paste");
        }
        if mode.intersects(Modes::MOUSE_MODE) {
            dispatch_context.add("any_mouse_reporting");
        }
        {
            let mouse_reporting = if mode.contains(Modes::MOUSE_REPORT_CLICK) {
                "click"
            } else if mode.contains(Modes::MOUSE_DRAG) {
                "drag"
            } else if mode.contains(Modes::MOUSE_MOTION) {
                "motion"
            } else {
                "off"
            };
            dispatch_context.set("mouse_reporting", mouse_reporting);
        }
        {
            let format = if mode.contains(Modes::SGR_MOUSE) {
                "sgr"
            } else if mode.contains(Modes::UTF8_MOUSE) {
                "utf8"
            } else {
                "normal"
            };
            dispatch_context.set("mouse_format", format);
        };

        if self.terminal.read(cx).last_content.selection.is_some() {
            dispatch_context.add("selection");
        }

        dispatch_context
    }

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .terminal
            .read(cx)
            .last_content
            .mode
            .contains(Modes::ALT_SCREEN)
        {
            self.terminal.update(cx, |term, cx| {
                term.try_keystroke(
                    &Keystroke::parse("ctrl-cmd-space").unwrap(),
                    TerminalSettings::get_global(cx).option_as_meta,
                )
            });
        } else {
            window.show_character_palette();
        }
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |term, _| term.select_all());
        cx.notify();
    }
}
