//! AA Terminal Backend — 纯 PTY + alacritty 模拟器，零 GPUI 依赖。
//!
//! 渲染层（Element + View）在 `aa-terminal-view` crate.

// re-export settings_content 的 merge_from / fallible_options 供 settings_macros 宏使用。
// settings-macros 的 #[derive(MergeFrom)] 和 #[with_fallible_options] 会生成
// `crate::merge_from::MergeFrom` 和 `crate::fallible_options::deserialize`。
mod merge_from {
    pub use settings_content::merge_from::*;
}
mod fallible_options {
    pub use settings_content::fallible_options::*;
}

pub mod alacritty;
pub mod settings;

pub use alacritty::{AlacrittyBackend, DisplayCell, DisplayCursor, TerminalBounds};
