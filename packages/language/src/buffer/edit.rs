use super::*;
use std::{
    any::Any,
    cell::Cell,
    cmp::{self, Ordering, Reverse},
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    future::Future,
    iter::{self, Iterator, Peekable},
    mem,
    num::NonZeroU32,
    ops::{Deref, Range},
    path::PathBuf,
    rc,
    sync::Arc,
    time::{Duration, Instant},
    vec,
};
pub use text::{
    Anchor, Bias, Buffer as TextBuffer, BufferId, BufferSnapshot as TextBufferSnapshot, Edit,
    LineIndent, OffsetRangeExt, OffsetUtf16, Patch, Point, PointUtf16, Rope, Selection,
    SelectionGoal, Subscription, TextDimension, TextSummary, ToOffset, ToOffsetUtf16, ToPoint,
    ToPointUtf16, Transaction, TransactionId, Unclipped,
};

#[derive(Clone, Debug)]
pub enum AutoindentMode {
    /// Indent each line of inserted text.
    EachLine,
    /// Autoindent multiline edits, but only apply syntax-triggered outdents to single-line edits.
    PreserveSingleLine,
    /// Apply the same indentation adjustment to all of the lines
    /// in a given insertion.
    Block {
        /// The original indentation column of the first line of each
        /// insertion, if it has been copied.
        ///
        /// Knowing this makes it possible to preserve the relative indentation
        /// of every line in the insertion from when it was copied.
        ///
        /// If the original indent column is `a`, and the first line of insertion
        /// is then auto-indented to column `b`, then every other line of
        /// the insertion will be auto-indented to column `b - a`
        original_indent_columns: Vec<Option<u32>>,
    },
}

#[derive(Clone)]
pub(crate) struct AutoindentRequest {
    pub(crate) before_edit: BufferSnapshot,
    pub(crate) entries: Vec<AutoindentRequestEntry>,
    pub(crate) is_block_mode: bool,
    pub(crate) ignore_empty_lines: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AutoindentRequestEntry {
    /// A range of the buffer whose indentation should be adjusted.
    pub(crate) range: Range<Anchor>,
    /// The row of the edit start in the buffer before the edit was applied.
    /// This is stored here because the anchor in range is created after
    /// the edit, so it cannot be used with the before_edit snapshot.
    pub(crate) old_row: Option<u32>,
    pub(crate) indent_size: IndentSize,
    pub(crate) original_indent_column: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct IndentSuggestion {
    pub(crate) basis_row: u32,
    pub(crate) delta: Ordering,
    pub(crate) within_error: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum AutoIndentExclusion {
    PrecedingLine,
    FollowingLine,
}
