//! TerminalProvider trait 实现 — TerminalPanelProvider。
//!
//! workspace crate 定义了 `TerminalProvider` trait（抽象终端创建），
//! 但 trait 本身不依赖 terminal-view。terminal-view crate 提供具体实现。
//!
//! 对齐 Zed 分层：Zed 的 provider 实现类在 `terminal_view` 内部，
//! 通过 `Workspace::set_terminal_provider(Box::new(provider))` 注入。
//!
//! Workspace 目前还没有 terminal_provider 字段，注入入口后续添加。

use std::path::PathBuf;
use std::process::ExitStatus;

use gpui::{App, AppContext, Entity, Task, WeakEntity, Window};
use task::SpawnInTerminal;
use workspace::TerminalProvider;

use super::TerminalPanel;
use crate::view::TerminalView;

/// TerminalProvider 的 TerminalPanel 实现。
///
/// 持有 TerminalPanel 的 WeakEntity，spawn 时通过它创建新 terminal tab。
/// 对齐 Zed 的实现方式（provider 内部引用 terminal panel entity）。
pub struct TerminalPanelProvider {
    terminal_panel: WeakEntity<TerminalPanel>,
}

impl TerminalPanelProvider {
    pub fn new(panel: &Entity<TerminalPanel>) -> Self {
        Self {
            terminal_panel: panel.downgrade(),
        }
    }
}

impl TerminalProvider for TerminalPanelProvider {
    fn spawn(
        &self,
        task: SpawnInTerminal,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Result<ExitStatus, std::io::Error>>> {
        let Some(panel) = self.terminal_panel.upgrade() else {
            return Task::ready(None);
        };

        // 取 working directory：task 有 cwd 用 cwd，否则 fallback 当前目录
        let working_dir = task.cwd.clone().unwrap_or_else(current_dir);

        // 创建 TerminalView 加进 TerminalPanel 的 active_pane
        // 注：目前不真的 spawn PTY process，TerminalView::new 是空壳。
        // 未来接入 aa-terminal PTY backend 后，这里会 spawn 真正的 shell/command。
        panel.update(cx, |panel, cx| {
            let terminal = cx.new(|cx| {
                TerminalView::new(
                    task.command.clone(), // Option<String>，直接透传
                    working_dir,
                    cx,
                )
            });
            panel.active_pane.update(cx, |pane, cx| {
                pane.add_item(terminal, cx);
            });
        });

        // 返回一个 ready 的 ExitStatus（目前没有真实 PTY，直接 Ok）
        Task::ready(Some(Ok(ExitStatus::default())))
    }
}

fn current_dir() -> PathBuf { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")) }
