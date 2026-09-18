//! AA Terminal — 基于 alacritty_terminal 的 PTY 终端面板。
//!
//! 对齐 zed `crates/terminal`：
//! - `alacritty.rs` — AlacrittyBackend（PTY + EventLoop + cell grid）
//! - `element.rs` — TerminalElement（cell grid → pixel 渲染）
//! - `view.rs` — TerminalView（GPUI entity + 键盘输入）+ Panel trait impl

pub mod alacritty;
pub mod element;
pub mod view;

pub use view::{TerminalView, initial_bounds};
