// 子模块声明
mod bracket_ranges;
mod char_classifier;
mod chunks;
mod core;
mod edit;
mod edit_preview;
mod event;
mod file;
mod highlighted_text;
mod indent;
mod snapshot;
mod test_support; // #[cfg(any(test, feature="test-support"))]
mod util;
mod words;

pub mod row_chunk;
mod tree_sitter_data;
use aa_clock as clock;
// 对外导出
pub use bracket_ranges::BracketMatch;
pub use char_classifier::{CharClassifier, CharKind, CharScopeContext};
pub use chunks::{BufferChunks, Chunk, LanguageAwareStyling};
pub use core::{Buffer, EditedBufferSnapshot};
pub use edit::{AutoIndentExclusion, AutoindentMode};
pub use edit_preview::EditPreview;
pub use event::{BufferEditSource, BufferEvent, Operation, ParseStatus};
pub use file::{DiskState, File, LocalFile};
pub use highlighted_text::{HighlightedText, HighlightedTextBuilder};
pub use indent::{IndentKind, IndentSize};
pub use snapshot::BufferSnapshot;
pub use words::WordsQuery;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::TestFile;

// 共享类型
pub type BufferRow = u32;

/// Indicate whether a [`Buffer`] has permissions to edit.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Capability {
    ReadWrite,
    Read,
    ReadOnly,
}

impl Capability {
    pub fn editable(self) -> bool { matches!(self, Capability::ReadWrite) }
}

// 便于子模块用 `super::*` 访问（与原文件 layout 保持一致）
pub(crate) use event::DiagnosticEndpoint;
pub(crate) use util::{contiguous_ranges, offset_in_sub_ranges, trailing_whitespace_ranges};
