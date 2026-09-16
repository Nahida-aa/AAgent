use std::path::PathBuf;

use gpui::{AppContext, Context, Decorations, Entity, ParentElement, Render, Styled, prelude::*};
use ui_gpui::theme::ActiveTheme;

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
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        // 自绘 header 只在 CSD 生效时显示：Windows 恒显示（无系统标题栏可依赖）；
        // Wayland 由于 window_decorations=Client 强制 CSD；X11/SSD 交给系统标题栏
        //（否则 W 双层标题栏）。同 aa-player AppShell::render。
        let show_titlebar = cfg!(target_os = "windows")
            || matches!(window.window_decorations(), Decorations::Client { .. });

        gpui::div()
            .flex_col()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .when(show_titlebar, |root| root.child(self.title_bar.clone()))
            .child(self.panel.clone())
    }
}
