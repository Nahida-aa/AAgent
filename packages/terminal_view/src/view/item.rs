//! TerminalView 作为 Pane 里的 Item。

use aa_gpui_kit_ui::IconName;
use gpui::App;
use workspace::Item;

use super::TerminalView;

impl Item for TerminalView {
    type Event = ();

    fn tab_label(&self, _cx: &App) -> gpui::SharedString {
        "Terminal".into()
    }

    fn tab_icon(&self, _cx: &App) -> IconName {
        IconName::TerminalAlt
    }
}
