//! AA Terminal View — GPUI 渲染层（element + view + panel）。
//!
//! backend 在 [`aa-terminal`] crate（纯 PTY + alacritty 模拟器，零 GPUI 依赖）。
//! 这里有：
//! - element::TerminalElement — 纯绘制
//! - view::TerminalView — Item entity（装在 Pane 里）
//! - panel::TerminalPanel — Dock Panel（装在底部 Dock 里，内含 active_pane）
pub mod element;
pub mod panel;
mod persistence;
pub mod terminal_scrollbar;
pub mod view;

pub use panel::TerminalPanel;
pub use view::TerminalView;

use gpui::App;
use workspace::{Workspace, item::register_serializable_item};

pub fn init(cx: &mut App) {
    terminal_panel::init(cx);

    register_serializable_item::<TerminalView>(cx);

    cx.observe_new(|workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(TerminalView::deploy);
    })
    .detach();
}
