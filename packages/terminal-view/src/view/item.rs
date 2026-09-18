//! TerminalView 作为 Pane 里的 Item。
//!
//! Zed 在 `terminal_view.rs` 里直接 `impl workspace::item::Item for TerminalView`。
//! AAgent 的 Item trait 简化为两个方法：tab_label / tab_icon。

use gpui::App;
use ui_gpui::IconName;
use workspace::Item;

use super::TerminalView;

impl Item for TerminalView {
    fn tab_label(&self, _cx: &App) -> gpui::SharedString {
        "Terminal".into()
    }

    fn tab_icon(&self, _cx: &App) -> IconName {
        IconName::TerminalAlt
    }
}
