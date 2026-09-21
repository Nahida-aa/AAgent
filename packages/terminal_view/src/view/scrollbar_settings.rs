use editor::{EditorSettings, ui_scrollbar_settings_from_raw};
use gpui::App;
use settings::Settings;
use terminal::terminal_settings::TerminalSettings;
use ui::scrollbars::{self, ScrollbarVisibility};

#[derive(Default)]
pub(super) struct TerminalScrollbarSettingsWrapper;

impl ScrollbarVisibility for TerminalScrollbarSettingsWrapper {
    fn visibility(&self, cx: &App) -> scrollbars::ShowScrollbar {
        TerminalSettings::get_global(cx)
            .scrollbar
            .show
            .map(ui_scrollbar_settings_from_raw)
            .unwrap_or_else(|| EditorSettings::get_global(cx).scrollbar.show)
    }
}
