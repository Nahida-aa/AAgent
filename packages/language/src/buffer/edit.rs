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
    EachLine,
    Block {
        original_indent_columns: Vec<Option<u32>>,
    },
}

#[derive(Clone)]
pub(crate) struct AutoindentRequest {
    before_edit: BufferSnapshot,
    entries: Vec<AutoindentRequestEntry>,
    is_block_mode: bool,
    ignore_empty_lines: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AutoindentRequestEntry {
    /// A range of the buffer whose indentation should be adjusted.
    range: Range<Anchor>,
    /// The row of the edit start in the buffer before the edit was applied.
    /// This is stored here because the anchor in range is created after
    /// the edit, so it cannot be used with the before_edit snapshot.
    old_row: Option<u32>,
    indent_size: IndentSize,
    original_indent_column: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct IndentSuggestion {
    basis_row: u32,
    delta: Ordering,
    within_error: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum AutoIndentExclusion {
    PrecedingLine,
    FollowingLine,
}
