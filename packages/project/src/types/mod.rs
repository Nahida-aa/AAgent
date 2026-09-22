//! Project 里用到的 LSP / 补全相关数据类型。
//!
//! zed 把这些类型直接定义在 `crates/project/src/project.rs` 里；这里按用途
//! 拆成子模块。为保持 zed 单文件里那种「裸名随手可用」的手感，本模块集中
//! 引入这些类型，子模块统一 `use super::*;` 取用。

mod code_action;
mod completion;
mod diagnostics;
mod document_color;
mod hover;
mod inlay_hint;
mod location;
mod prepare_rename;
mod symbol;

pub use code_action::{CodeAction, LspAction};
pub(crate) use completion::{CoreCompletion, CoreCompletionResponse};
pub use completion::{
    Completion, CompletionDisplayOptions, CompletionGroup, CompletionIntent, CompletionResponse,
    CompletionSource,
};
pub use diagnostics::{LspPullDiagnostics, PulledDiagnostics};
pub use document_color::{ColorPresentation, DocumentColor};
pub use hover::{Hover, HoverBlock, HoverBlockKind};
pub use inlay_hint::{
    InlayHint, InlayHintLabel, InlayHintLabelPart, InlayHintLabelPartTooltip, InlayHintTooltip,
    InlayId, MarkupContent, ResolveState,
};
pub use location::{DocumentHighlight, LocationLink};
pub use prepare_rename::PrepareRenameResponse;
pub use symbol::{DocumentSymbol, Symbol};

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use gpui::{App, Hsla, SharedString, Window};
use language::{
    Anchor, CodeLabel, Language, Location, Rope, ToOffset, Unclipped,
    language_settings::InlayHintKind,
};
use lsp::{DocumentHighlightKind, InsertTextMode, LanguageServerId, LanguageServerName};

use crate::buffer_store::BufferStore;
use crate::debugger::breakpoint_store::BreakpointStore;
use crate::debugger::dap_store::DapStore;
use crate::git_store::GitStore;
use crate::lsp_store::{CompletionDocumentation, LspStore, SymbolLocation};
use crate::project::Project;
use crate::project_settings::SettingsObserver;
use crate::worktree_store::WorktreeStore;
use client::PendingEntitySubscription;
use language::PointUtf16;
use worktree::WorktreeId;
