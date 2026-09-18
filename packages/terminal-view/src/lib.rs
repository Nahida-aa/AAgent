//! AA Terminal View — GPUI 渲染层（element + view）。
//!
//! backend 在 [`aa-terminal`] crate（纯 PTY + alacritty 模拟器，零 GPUI 依赖）。
//! 这里只有 GPUI Element trait impl 和 TerminalView entity。

pub mod element;
pub mod view;

pub use view::{TerminalView, initial_bounds};
