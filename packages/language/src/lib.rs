//! The `language` crate provides a large chunk of Zed's language-related
//! features ... （保持原文档注释）

mod available_languages;
mod buffer;
mod diagnostic;
mod diagnostic_set;
mod file_content;
mod language_registry;

pub mod language_settings;
mod manifest;
pub mod modeline;
mod outline;
pub mod proto;
pub use diagnostic_set::DiagnosticSet;
mod runnable;
mod syntax_map;
mod task_context;
mod text_diff;
mod toolchain;

// 新增拆分出来的模块
#[cfg(test)]
pub mod buffer_tests;
mod highlight;
mod language;
mod lsp_adapter;
mod parser_pool;
mod plain_text;
mod point_range;
#[cfg(test)]
mod proto_diagnostics_tests;
mod symbol_kind;
#[cfg(test)]
mod tests;

// 让 `clock::` 在整个 crate 内可用（对齐 zed 中名为 `clock` 的外部 crate）。

pub use crate::language_settings::{
    AutoIndentMode, EditPredictionPromptFormat, EditPredictionsMode, IndentGuideSettings,
    ZetaVersion,
};
pub use language_core::{
    BlockCommentConfig, BracketPair, BracketPairConfig, BracketPairContent, BracketsConfig,
    BracketsPatternConfig, CodeLabel, CodeLabelBuilder, DebugVariablesConfig, DebuggerTextObject,
    DecreaseIndentConfig, Grammar, GrammarId, HighlightsConfig, IndentConfig, InjectionConfig,
    InjectionPatternConfig, JsxTagAutoCloseConfig, LanguageConfig, LanguageConfigOverride,
    LanguageId, LanguageMatcher, OrderedListConfig, OutlineConfig, Override, OverrideConfig,
    OverrideEntry, RedactionConfig, RunnableCapture, RunnableConfig, SoftWrap, Symbol,
    TaskListConfig, TextObject, TextObjectConfig, WrapCharactersConfig, default_true,
    deserialize_regex, deserialize_regex_vec, regex_json_schema, regex_vec_json_schema,
    serialize_regex,
};
pub use language_core::{
    SymbolKind,
    highlight_cache::ResolvedHighlights,
    highlight_map::{CaptureId, HighlightId, HighlightMap},
};
pub use language_registry::{
    BinaryStatus, LanguageLoader, LanguageName, LanguageNotFound, LanguageQueries,
    LanguageRegistry, LanguageServerStatusUpdate, LoadedLanguage, QueryFile, QueryFileContents,
    QueryFiles, ServerHealth,
};
pub use manifest::{ManifestDelegate, ManifestName, ManifestProvider, ManifestQuery};
pub use modeline::{ModelineSettings, parse_modeline};
pub use outline::*;
pub use runnable::{
    ResolvedRunnable, Runnable, RunnableMatchCapture, RunnableRange, RunnableResolver, RunnableTag,
};
pub use syntax_map::{
    OwnedSyntaxLayer, SyntaxLayer, SyntaxMapMatches, ToTreeSitterPoint, TreeSitterOptions,
};
pub use task_context::{ContextLocation, ContextProvider};
pub use text_diff::{
    Diff, DiffOptions, apply_diff_patch, apply_reversed_diff_patch, char_diff, line_diff,
    text_diff, text_diff_with_options, unified_diff, unified_diff_with_context,
    unified_diff_with_offsets, word_diff_ranges,
};
pub use toolchain::{
    LanguageToolchainStore, LocalLanguageToolchainStore, Toolchain, ToolchainList, ToolchainLister,
    ToolchainMetadata, ToolchainScope,
};

pub use available_languages::AvailableLanguage;
pub use buffer::Operation;
pub use buffer::snapshot::CursorShape;
pub use buffer::*;
pub use diagnostic::{
    Diagnostic, DiagnosticMessage, DiagnosticSourceKind, RelatedInformation, RelatedLocation,
};
pub use diagnostic_set::{DiagnosticEntry, DiagnosticEntryRef, DiagnosticGroup};
pub use file_content::{
    ByteContent, DecodedText, FILE_ANALYSIS_BYTES, analyze_byte_content, decode_text, encode_text,
};
pub use lsp::{LanguageServerId, LanguageServerName};
pub use buffer::TextBufferSnapshot;
pub use text::{
    Anchor, AnchorRangeExt, Bias, BufferId, Edit, LineEnding, OffsetRangeExt, OffsetUtf16, Patch,
    Point, PointUtf16, Rope, Selection, SelectionGoal, ToOffset, ToOffsetUtf16, ToPoint,
    ToPointUtf16, Transaction, TransactionId, Unclipped,
};
pub use tree_sitter::{Node, Parser, QueryCapture, Tree, TreeCursor};

// 拆分出来的模块的公开导出
pub use crate::highlight::build_highlight_map;
pub use crate::language::{CodeLabelExt, Language, LanguageScope};
pub use crate::lsp_adapter::{
    CachedLspAdapter, ClientCommand, DownloadableLanguageServerBinary, DynLspInstaller,
    LanguageServerBinaryLocations, Location, LspAdapter, LspAdapterDelegate, LspInstaller,
    PromptResponseContext, ServerBinaryCache,
};
pub use crate::parser_pool::{parse_text, with_parser, with_query_cursor};
pub use crate::plain_text::PLAIN_TEXT;
pub use crate::point_range::{point_from_lsp, point_to_lsp, range_from_lsp, range_to_lsp};
pub use crate::symbol_kind::{lsp_to_symbol_kind, symbol_kind_to_lsp};

#[cfg(any(test, feature = "test-support"))]
pub use crate::test_support::{FakeLspAdapter, json_lang, markdown_lang, rust_lang};

pub(crate) fn to_settings_soft_wrap(value: language_core::SoftWrap) -> settings::SoftWrap {
    match value {
        language_core::SoftWrap::None => settings::SoftWrap::None,
        language_core::SoftWrap::PreferLine => settings::SoftWrap::PreferLine,
        language_core::SoftWrap::EditorWidth => settings::SoftWrap::EditorWidth,
        language_core::SoftWrap::Bounded => settings::SoftWrap::Bounded,
    }
}

pub(crate) use parser_pool::QUERY_CURSORS;
