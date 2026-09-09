use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, ParentElement, Render, Styled};

use crate::terminal::TerminalView;

/// AAgent desktop app shell.
pub struct AppShell {
    terminal: Entity<TerminalView>,
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let terminal = cx.new(|cx| TerminalView::new(None, working_dir, cx));
        Self { terminal }
    }
}

impl Render for AppShell {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
            .flex_grow(1.0)
            .bg(gpui::rgb(0x0d1117))
            .child(self.terminal.clone())
    }
}
