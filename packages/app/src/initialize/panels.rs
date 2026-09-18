//! 统一初始化所有 Dock Panel。
//!
//! 对齐 Zed `crates/zed/src/zed.rs::initialize_panels`。
//! Zed 版用 async 加载 + 持久化恢复；AAgent 简化为同步创建，
//! 但保持 `Task<anyhow::Result<()>>` 签名以便后续扩展（持久化/异步加载）。

use aa_terminal_view::TerminalPanel;
use anyhow::Result;
use gpui::{AppContext, Context, Task, Window};
use workspace::Workspace;
use workspace::dock::panel::{
    AgentPanel, CollabPanel, DebugPanel, GitPanel, OutlinePanel, ProjectPanel,
};

/// 创建所有 Panel entity 并注入到 Workspace 的对应 Dock。
///
/// Workspace::new() 只创建空 Dock（对齐 Zed），面板统一在这里组装：
///
/// ```text
/// Left Dock   → AgentPanel
/// Right Dock  → ProjectPanel, GitPanel, CollabPanel, OutlinePanel
/// Bottom Dock → TerminalPanel, DebugPanel
/// ```
///
/// Zed 对应：`zed.rs:776 initialize_panels` — 用 `futures::join!` 并行 async load，
/// 因为每个 Panel::load() 要恢复持久化状态（KV store）。
/// AAgent 当前无持久化，`cx.new()` 是同步的，所以直接创建 + 同步 add_panel。
pub fn initialize_panels(_window: &mut Window, cx: &mut Context<Workspace>) -> Task<Result<()>> {
    let workspace_entity = cx.entity();

    cx.update_entity(&workspace_entity, |workspace, cx| {
        // —— Left Dock ——
        let agent = cx.new(|_| AgentPanel);
        workspace.add_panel::<AgentPanel>(agent, cx);

        // —— Right Dock ——
        let project = cx.new(|_| ProjectPanel);
        workspace.add_panel::<ProjectPanel>(project, cx);

        let git = cx.new(|_| GitPanel);
        workspace.add_panel::<GitPanel>(git, cx);

        let collab = cx.new(|_| CollabPanel);
        workspace.add_panel::<CollabPanel>(collab, cx);

        let outline = cx.new(|_| OutlinePanel);
        workspace.add_panel::<OutlinePanel>(outline, cx);

        // —— Bottom Dock ——
        let terminal = cx.new(|cx| TerminalPanel::new(cx));
        workspace.add_panel::<TerminalPanel>(terminal, cx);

        let debug = cx.new(|_| DebugPanel);
        workspace.add_panel::<DebugPanel>(debug, cx);

        cx.notify();
    });

    // 未来：持久化恢复、并行加载、panel 级别的 deferred tasks
    // 对齐 Zed: workspace.finish_dock_restoration(cx)
    Task::ready(Ok(()))
}
