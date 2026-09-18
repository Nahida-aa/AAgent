use std::path::PathBuf;

use crate::initialize::panels::initialize_panels;
use aa_sidebar::Sidebar;
use aa_title_bar::TitleBar;
use gpui::{Context, Decorations, Entity, ParentElement, Render, Styled, Window, prelude::*};
use ui_gpui::theme::ActiveTheme;
use workspace::{MultiWorkspace, Workspace};

/// AAgent desktop app shell。对齐 zed 的 App → MultiWorkspace → Workspace → Dock/StatusBar 组装方式。
///
/// 层级：
/// ```
/// AppShell
/// ├── TitleBar (CSD/SSD 条件渲染)
/// └── MultiWorkspace
///     ├── [Sidebar?] ← Agent Threads 列表（独立于 Dock）
///     └── Workspace
///         ├── Left Dock (Agent)
///         ├── Center Pane (placeholder)
///         ├── Right Dock (Project, Git, Collab, Outline)
///         ├── Bottom Dock (Terminal, Debug)
///         └── StatusBar (PanelButtons + 普通项 + Sidebar toggle)
///     └── [Sidebar?]
/// ```
pub struct AppShell {
    title_bar: Entity<TitleBar>,
    multi_workspace: Entity<MultiWorkspace>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace = cx.new(|cx| Workspace::new(cx));
        let title_bar = cx.new(|cx| TitleBar::new("AAgent", workspace.clone(), cx));
        let multi_workspace = cx.new(|cx| MultiWorkspace::new(workspace.clone(), cx));

        // 1. 创建 Sidebar entity（独立于 workspace crate 的内容容器）
        let sidebar = cx.new(|cx| Sidebar::new(cx));

        // 2. Sidebar 绑定 MultiWorkspace 弱引用 — 底部栏关闭按钮回调需要
        sidebar.update(cx, |s, cx| {
            s.set_multi_workspace(multi_workspace.clone());
            cx.notify();
        });

        // 3. 注入 MultiWorkspace — Entity<Sidebar> → Box<dyn SidebarHandle>（bridge impl）
        multi_workspace.update(cx, |mw, cx| {
            mw.set_sidebar(Box::new(sidebar.clone()), cx);
            cx.notify();
        });

        // 4. Workspace/StatusBar 绑定 MultiWorkspace — Sidebar toggle 走 MultiWorkspace 中转
        workspace.update(cx, |w, cx| {
            w.set_multi_workspace(multi_workspace.clone(), cx);
            cx.notify();
        });

        // 5. 统一初始化所有 Dock Panel（对齐 Zed initialize_panels）
        // Workspace::new() 只创建空 Dock，面板统一在这里注入
        workspace.update(cx, |_workspace, cx| {
            let _ = initialize_panels(window, cx);
        });

        Self {
            title_bar,
            multi_workspace,
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
            .child(self.multi_workspace.clone())
    }
}
