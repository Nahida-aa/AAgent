//! TerminalPanel — 底部 Dock 里的终端面板。
//!
//! 对齐 Zed `crates/terminal_view/src/terminal_panel.rs`。
//! Zed 版持有完整 PaneGroup + 多 pane + 持久化 + action handler。
//! AAgent 最小版：一个 active_pane，每次 new_terminal() 创建 TerminalView 加进去。

use gpui::{App, AppContext, Context, Entity, IntoElement, Render, Window, px};
use ui_gpui::IconName;
use workspace::DockPosition;
use workspace::Pane;
use workspace::dock::panel::Panel;

use crate::view::TerminalView;

/// 底部终端面板 — 一个 Dock Panel，内部持有一个 Pane。
///
/// ```text
/// Dock (bottom)
///  └── TerminalPanel (render 这个)
///       └── active_pane: Entity<Pane>
///            └── items: Vec<Box<dyn ItemHandle>>  ← 每个都是 TerminalView
/// ```
pub struct TerminalPanel {
    active_pane: Entity<Pane>,
}

impl TerminalPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let active_pane = cx.new(Pane::new);
        Self { active_pane }
    }

    /// 创建一个新的 TerminalView 加进 active_pane。
    pub fn new_terminal(&mut self, cx: &mut Context<Self>) {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let terminal = cx.new(|cx| TerminalView::new(None, working_dir, cx));
        self.active_pane.update(cx, |pane, cx| {
            pane.add_item(terminal, cx);
        });
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.active_pane.clone().into_any_element()
    }
}

impl Panel for TerminalPanel {
    fn panel_key() -> &'static str {
        "terminal"
    }

    fn persistent_name() -> &'static str {
        "terminal"
    }

    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Bottom
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        // Terminal 可以在任何 dock 位置（Zed 里 Terminal/Debug 都这样）
        true
    }

    fn default_size(&self, _cx: &App) -> gpui::Pixels {
        px(320.0)
    }

    fn icon(&self, _cx: &App) -> IconName {
        IconName::TerminalAlt
    }

    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Terminal Panel"
    }
}
