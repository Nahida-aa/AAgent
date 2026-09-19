//! TerminalPanel — 底部 Dock 里的终端面板。
//!
//! 对齐 Zed `crates/terminal_view/src/terminal_panel.rs`。
//! Zed 版持有完整 PaneGroup + 多 pane + 持久化 + action handler。
//! AAgent 最小版：一个 active_pane，每次 new_terminal() 创建 TerminalView 加进去。

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, IntoElement, Render, WeakEntity, Window,
    actions, px,
};
use ui_gpui::IconName;
use workspace::dock::panel::Panel;
use workspace::{DockPosition, Pane, PaneEvent, Workspace};

use crate::view::TerminalView;

// ---------- TerminalPanel 自身 action ----------

actions!(terminal_panel, [Toggle, ToggleFocus]);

// ---------- TerminalPanel entity ----------

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
    workspace: WeakEntity<Workspace>,
    /// 对齐 zed TerminalPanel::active — Dock 打开/关闭时由 Dock 调用 set_active 设置。
    /// true 时如果 pane 为空 → spawn 默认 terminal。
    active: bool,
}

impl EventEmitter<()> for TerminalPanel {}

impl TerminalPanel {
    pub fn new(workspace: &Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let active_pane = cx.new(Pane::new);

        // 订阅 Pane 事件 — 当最后一个 tab 被关闭 (Event::Empty) 时关闭 Dock
        // 对齐 Zed terminal_panel.rs:L438-445 — 监听 pane::Event::Remove
        cx.subscribe(&active_pane, &TerminalPanel::handle_pane_event)
            .detach();

        let mut panel = Self {
            active_pane,
            workspace: workspace.downgrade(),
            active: false,
        };

        // 对齐 Zed finish_restoration — 没有持久化时 spawn 默认 shell
        panel.spawn_default_terminal(cx);

        panel
    }

    fn has_no_terminals(&self, cx: &App) -> bool {
        self.active_pane.read(cx).items().is_empty()
    }

    fn handle_pane_event(
        &mut self,
        _pane: Entity<Pane>,
        event: &PaneEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            // 对齐 Zed terminal_panel.rs:L438-445 — 最后一个 pane → emit Close
            // 我们简化为：一个 pane 变空 → 关闭整个 TerminalPanel Dock
            PaneEvent::Empty => {
                if let Some(workspace) = self.workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        workspace.close_panel::<Self>(cx);
                    });
                }
            }
            _ => {}
        }
    }

    /// 创建一个新的 TerminalView 加进 active_pane。
    pub fn new_terminal(&mut self, cx: &mut Context<Self>) {
        self.spawn_default_terminal(cx);
    }

    fn spawn_default_terminal(&mut self, cx: &mut Context<Self>) {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let terminal = cx.new(|cx| TerminalView::new(None, working_dir, cx));
        self.active_pane.update(cx, |pane, cx| {
            pane.add_item(terminal, cx);
        });
    }

    // ---------- Zed init() 注册 action handler ----------

    /// App 层启动时调用 — 注册 Workspace 级别的 action handler。
    ///
    /// 对齐 zed `terminal_panel::init(cx)` (terminal_panel.rs:55)。
    /// 用 `cx.observe_new` 在每个 Workspace 创建时注册 handler：
    /// - workspace::NewTerminal → TerminalPanel::new_terminal
    /// - TerminalPanel::ToggleFocus → toggle_panel_focus::<TerminalPanel>
    pub fn init(cx: &mut App) {
        cx.observe_new(|workspace: &mut workspace::Workspace, _window, cx| {
            // NewTerminal → TerminalPanel::new_terminal + open panel
            workspace.register_action(|workspace, _: &workspace::NewTerminal, _, cx| {
                // 通过 Workspace::panel::<TerminalPanel>(cx) 拿到 entity
                if let Some(terminal_panel) = workspace.panel::<Self>(cx) {
                    terminal_panel.update(cx, |panel, cx| {
                        panel.new_terminal(cx);
                    });
                    workspace.open_panel::<Self>(cx);
                }
            });

            // ToggleFocus — 打开/关闭 TerminalPanel 的 Dock
            workspace.register_action(|workspace, _: &ToggleFocus, _, cx| {
                workspace.toggle_panel_focus::<Self>(cx);
            });

            // Toggle — toggle，已开则 close
            workspace.register_action(|workspace, _: &Toggle, _, cx| {
                let opened = workspace.toggle_panel_focus::<Self>(cx);
                if !opened {
                    workspace.close_panel::<Self>(cx);
                }
            });
        })
        .detach();
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

    /// 对齐 zed terminal_panel.rs:L1745-1761 — active 且 pane 空 → spawn 默认 shell
    fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        if active && self.has_no_terminals(cx) {
            self.spawn_default_terminal(cx);
        }
    }

    /// 对齐 Zed TerminalPanel — 从 settings 读，默认 false。
    /// 现在还没 settings 系统，先硬编码 false。
    fn starts_open(&self, _cx: &App) -> bool {
        false
    }
}
