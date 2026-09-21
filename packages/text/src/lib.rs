mod anchor;
pub mod locator;
#[cfg(any(test, feature = "test-support"))]
pub mod network;
mod patch;
mod selection;
pub mod subscription;
mod undo_map;

// 新增拆分
mod buffer;
mod buffer_id;
mod constants;
#[cfg(debug_assertions)]
pub mod debug_ranges;
mod dimensions;
mod edit_snapshot;
mod fragment;
mod history;
mod line;
mod offset_traits; // 也可叫 offset_traits
mod operation;
mod rope_builder;
mod snapshot;
#[cfg(any(test, feature = "test-support"))]
mod test_support;

#[cfg(test)]
mod tests;

use aa_clock as clock;
pub use anchor::*;
pub use buffer::Buffer;
pub use buffer_id::BufferId;
pub use constants::MAX_INSERTION_LEN;
pub use debug::GlobalDebugRanges;
pub use edit_snapshot::EditedBufferSnapshot;
pub use edit_traits::{FromAnchor, ToOffset, ToOffsetUtf16, ToPoint, ToPointUtf16};
pub use fragment::{Fragment, FragmentSummary};
pub use history::{HistoryEntry, Transaction};
pub use line::{LineEnding, LineIndent, chunks_with_line_ending};
pub use operation::{Edit, EditOperation, Operation, UndoOperation};
pub use patch::Patch;
use regex::Regex;
pub use snapshot::BufferSnapshot;
use std::sync::{Arc, LazyLock};

pub(crate) static LINE_SEPARATORS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\r\n|\r").expect("Failed to create LINE_SEPARATORS_REGEX"));

// 共享类型别名
pub type TransactionId = clock::Lamport;

// 内部共享用的类型
pub(crate) use dimensions::{FullOffset, VersionedFullOffset as VFO};
pub(crate) use fragment::FragmentBuilder;
pub(crate) use fragment::FragmentChunk;
pub(crate) use fragment::FragmentTextSummary;
pub(crate) use fragment::InsertionFragment;
pub(crate) use fragment::InsertionFragmentKey;
pub(crate) use fragment::InsertionSlice;
pub(crate) use fragment::VersionedFullOffset;
pub(crate) use rope_builder::RopeBuilder;
