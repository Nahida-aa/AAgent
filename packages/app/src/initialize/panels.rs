//! Dock Panel 统一注册 — 对齐 Zed `crates/zed/src/zed.rs::initialize_panels`。
//!
//! Zed 的做法：每个 Panel crate 有 `init(cx)` 注册 action（toggle/focus），
//! 然后在某个时机统一调用 `Panel::load()` + `Workspace::add_panel()`。
//!
//! AAgent 当前只有 `project_panel` + `outline_panel` 两个 Panel crate，
//! AgentPanel / GitPanel / CollabPanel / DebugPanel 等尚未创建。
//! 先保留此函数签名，后续逐个接入。

use gpui::Entity;
use std::time::Duration;
use workspace::Workspace;

/// 在 Workspace observe_new 回调里被调用，负责把各 Panel entity 注入 Dock。
pub fn initialize_panels(window: &mut gpui::Window, workspace: &Entity<Workspace>, cx: &mut gpui::App) {
    let _ = window;
    let _ = workspace;
    let _ = cx;
    // TODO: 逐个接入 Panel crate：
    //   - project_panel::ProjectPanel::load(...) → ws.add_panel(...)
    //   - outline_panel::OutlinePanel::load(...) → ws.add_panel(...)
    //   - agent_ui::AgentPanel::load(...) → ws.add_panel(...)
    //   - git_ui::GitPanel::load(...) → ws.add_panel(...)
    //   - terminal_view::TerminalPanel::load(...) → ws.add_panel(...)
    //   - debug_panel::DebugPanel::load(...) → ws.add_panel(...)
    //
    // 对齐 Zed 用 futures::join! 并行加载，AAgent 当前无持久化需求可同步创建。
    //
    // 注意：各 Panel crate 的 `init(cx)` 已经在 `core::init` 里注册了 toggle/focus action，
    // 这里只需要创建 entity 并 add_panel。

    // 预留一个 future.detach() 占位，后续接入 futures::join!
    let _ = tokio::time::sleep(Duration::ZERO);
}
