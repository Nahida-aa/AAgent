use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, ParentElement, Render, Styled};

use crate::agent_panel::AgentPanel;

/// AAgent desktop app shell.
pub struct AppShell {
    panel: Entity<AgentPanel>,
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let panel = cx.new(|cx| AgentPanel::new(cx, working_dir));
        Self { panel }
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
            .bg(gpui::rgb(0x141417))
            .child(self.panel.clone())
    }
}
