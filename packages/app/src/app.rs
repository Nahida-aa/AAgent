use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, ParentElement, Render, Styled};

use crate::agent_panel::AgentPanel;
use crate::title_bar::TitleBar;

/// AAgent desktop app shell.
pub struct AppShell {
    title_bar: Entity<TitleBar>,
    panel: Entity<AgentPanel>,
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let title_bar = cx.new(|cx| TitleBar::new("AAgent", cx));
        let panel = cx.new(|cx| AgentPanel::new(cx, working_dir));
        Self { title_bar, panel }
    }
}

impl Render for AppShell {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x141417))
            .child(self.title_bar.clone())
            .child(self.panel.clone())
    }
}
