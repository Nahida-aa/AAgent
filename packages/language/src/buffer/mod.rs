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
mod test_support; // #[cfg(any(test, feature="test-support"))]
mod util;
mod words;

pub mod row_chunk;
pub(crate) mod snapshot;
mod tree_sitter_data;
// 对外导出
pub use bracket_ranges::BracketMatch;
pub use char_classifier::{CharClassifier, CharKind, CharScopeContext};
pub use chunks::{BufferChunks, Chunk, HighlightRun, LanguageAwareStyling};
pub use core::Buffer;
pub use edit::{AutoIndentExclusion, AutoindentMode};
pub use edit_preview::EditPreview;
pub use event::{BufferEditSource, BufferEvent, Operation, ParseStatus};
pub use file::{DiskState, File, LocalFile};
pub use highlighted_text::{HighlightedText, HighlightedTextBuilder};
pub use indent::{IndentKind, IndentSize};
pub use snapshot::BufferSnapshot;
pub use words::WordsQuery;

/// Wrapper combining an edited text snapshot with a buffer snapshot
/// (mirrors zed's `crates/buffer` `EditedBufferSnapshot`).
pub struct EditedBufferSnapshot {
    pub text: text::EditedBufferSnapshot,
    pub snapshot: BufferSnapshot,
}

impl EditedBufferSnapshot {
    pub fn snapshot(&self) -> &BufferSnapshot { &self.snapshot }

    pub fn base_version(&self) -> &clock::Global { &self.text.base_version }
}

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

// 共享导入桥接：拆分的子模块通过 `use super::*` 复用（原 zed buffer.rs 顶部 import 块）。
// 注意：这里不 `use text::*`，因为 `text::Buffer` 与本地 `Buffer` 同名会冲突；改为显式列出所需项。
pub(crate) use clock::{Global, Lamport, ReplicaId};
pub(crate) use aa_gpui_kit_theme::SyntaxTheme;
pub(crate) use anyhow::{Context as _, Result};
pub(crate) use collections::HashMap;
pub(crate) use encoding_rs::Encoding;
pub(crate) use fs::MTime;
pub(crate) use futures::channel::oneshot;
pub(crate) use futures_lite::future::yield_now;
pub(crate) use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, HighlightStyle, SharedString, StyledText,
    Task, TextStyle,
};
pub(crate) use language_core::highlight_map::{CaptureId, HighlightId, HighlightMap};
pub(crate) use lsp::{DiagnosticSeverity, LanguageServerId};
pub(crate) use parking_lot::Mutex;
pub(crate) use settings::{SettingsStore, WorktreeId};
pub(crate) use smallvec::SmallVec;
pub(crate) use std::{
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
pub(crate) use sum_tree::TreeMap;
pub use text::{
    Anchor, Bias, BufferId, BufferSnapshot as TextBufferSnapshot, Edit, LineEnding, LineIndent,
    OffsetRangeExt, OffsetUtf16, Patch, Point, PointUtf16, Rope, Selection, SelectionGoal,
    Subscription, TextSummary, ToOffset, ToPoint, ToPointUtf16, Transaction, TransactionId,
    Unclipped,
};

// crate 内部跨模块类型
pub(crate) use crate::diagnostic_set::{
    DiagnosticEntry, DiagnosticEntryRef, DiagnosticGroup, DiagnosticSet,
};
pub(crate) use crate::file_content::{ByteContent, analyze_byte_content};
pub(crate) use crate::language::{Language, LanguageScope};
pub(crate) use crate::language_registry::LanguageRegistry;
pub(crate) use crate::language_settings::{AutoIndentMode, LanguageSettings};
pub(crate) use crate::modeline::ModelineSettings;
pub(crate) use crate::outline::{Outline, OutlineItem};
pub(crate) use crate::plain_text::PLAIN_TEXT;
pub(crate) use crate::runnable::{self, Runnable, RunnableRange, RunnableTag};
pub(crate) use crate::syntax_map::{
    MAX_BYTES_TO_QUERY, SyntaxLayer, SyntaxMap, SyntaxMapCapture, SyntaxMapCaptures,
    SyntaxMapMatch, SyntaxMapMatches, SyntaxSnapshot, ToTreeSitterPoint, TreeSitterOptions,
    flattened_highlight_regions,
};
pub(crate) use crate::text_diff::{text_diff, unified_diff_with_offsets};
pub(crate) use language_core::{DebuggerTextObject, Grammar, TextObject};
pub(crate) use row_chunk::{RowChunkId, RowChunks};

// 子模块用到的、外部 crate 类型
pub(crate) use crate::Diff;
pub(crate) use crate::OutlineConfig;
pub(crate) use crate::ResolvedHighlights;
pub(crate) use crate::proto;
pub(crate) use language_core::highlight_cache::ChunkHighlightCache;
pub(crate) use path::PathStyle;
pub(crate) use path::rel_path::RelPath;
pub(crate) use rope::ChunkBitmaps;
pub(crate) use text::FromAnchor;

// 子模块用到的、定义在本模块树内的类型
pub(crate) use edit::{AutoindentRequestEntry, IndentSuggestion};
pub(crate) use indent::{indent_size_for_line, indent_size_for_text};
pub(crate) use tree_sitter_data::{MAX_BYTES_TO_HIGHLIGHT_IN_A_CHUNK, TreeSitterData};

// 子模块用到的、定义在本模块树内的公开类型
pub use snapshot::CursorShape;
