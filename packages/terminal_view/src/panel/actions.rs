use gpui::actions;

pub(super) const TERMINAL_PANEL_KEY: &str = "TerminalPanel";

actions!(
    terminal_panel,
    [
        /// Toggles the terminal panel.
        Toggle,
        /// Toggles focus on the terminal panel.
        ToggleFocus
    ]
);
