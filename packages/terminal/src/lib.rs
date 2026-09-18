//! AA Terminal Backend — 纯 PTY + alacritty 模拟器，零 GPUI 依赖。
//!
//! 渲染层（Element + View）在 `aa-terminal-view` crate。

pub mod alacritty;

pub use alacritty::{AlacrittyBackend, DisplayCell, DisplayCursor, TerminalBounds};
