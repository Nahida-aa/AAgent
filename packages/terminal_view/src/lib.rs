//! AA Terminal View — GPUI 渲染层（element + view + panel）。
//!
//! backend 在 [`aa-terminal`] crate（纯 PTY + alacritty 模拟器，零 GPUI 依赖）。
//! 模块结构对齐 Zed `crates/terminal_view/src/terminal_view.rs`（单文件 crate），
//! 只是拆成了目录：
//! - `element::TerminalElement` — 纯绘制（对应 Zed `terminal_element.rs`）
//! - `view::TerminalView` — Item entity（装在 Pane 里）
//! - `panel::TerminalPanel` — Dock Panel（装在底部 Dock 里）

pub mod element;
pub mod panel;
pub mod terminal_scrollbar;
pub mod view;

mod persistence;
mod terminal_path_like_target;

// ============================================================================
// 对齐 Zed 原版顶层 pub 导出（外部 crate 需要）
// ============================================================================
// Zed terminal_view.rs 直接定义的 pub item, aacode 拆成子模块后在这里转发。
// 内部 crate 访问: 子模块自己加 use 路径, 不通过这里。

// --- view 模块顶层 pub ---
pub use view::{
    ContentMode,
    TerminalMode,
    TerminalView,
    actions::{RenameTerminal, ScrollTerminal, SendKeystroke, SendText},
    block::{BlockContext, BlockProperties},
    working_directory::default_working_directory,
};

// --- panel 模块顶层 pub ---
pub use panel::TerminalPanel;

// --- element 模块顶层 pub ---
pub use element::{
    BatchedTextRun,
    BlockElementLayoutRect,
    LayoutPoint,
    LayoutRect,
    LayoutState,
    TerminalElement,
};

use gpui::App;
use workspace::{Workspace, register_serializable_item};

pub fn init(cx: &mut App) {
    panel::init(cx);

    register_serializable_item::<view::TerminalView>(cx);

    cx.observe_new(|workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(view::TerminalView::deploy);
    })
    .detach();
}
