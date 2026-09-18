use std::path::PathBuf;

use crate::title_bar::TitleBar;
use gpui::{Context, Decorations, Entity, ParentElement, Render, Styled, prelude::*};
use ui_gpui::theme::ActiveTheme;
use workspace::Workspace;

/// AAgent desktop app shell。对齐 zed 的 App → Workspace → Dock/StatusBar 组装方式。
///
/// 层级：
/// ```
/// AppShell
/// ├── TitleBar (CSD/SSD 条件渲染)
/// └── Workspace
///     ├── Left Dock (Project, Git)
///     ├── Center Pane (placeholder，后续接 AgentPanel)
///     ├── Right Dock (Collab, Outline, Debug, Agent)
///     ├── Bottom Dock (Terminal)
///     └── StatusBar (PanelButtons + 19 普通项)
/// ```
pub struct AppShell {
    title_bar: Entity<TitleBar>,
    workspace: Entity<Workspace>,
}

impl AppShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let _working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let title_bar = cx.new(|cx| TitleBar::new("AAgent", cx));
        let workspace = cx.new(|cx| Workspace::new(cx));
        Self {
            title_bar,
            workspace,
        }
    }
}

impl Render for AppShell {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        // 自绘 header 只在 CSD 生效时显示
        let show_titlebar = cfg!(target_os = "windows")
            || matches!(window.window_decorations(), Decorations::Client { .. });

        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .when(show_titlebar, |root| root.child(self.title_bar.clone()))
            .child(self.workspace.clone())
    }
}
