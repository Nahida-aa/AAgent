use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use anyhow::Result;
use gpui::{Context, Entity, Task};
use language::{Anchor, Buffer, Location, PointUtf16, ToOffset, ToPointUtf16, Transaction, Unclipped};
use lsp::{CodeActionKind, CompletionContext, LanguageServerId, LanguageServerName};
use lsp_command::*;
use lsp_store::{CompletionDocumentation, LspFormatTarget, OpenLspBufferHandle};
use text::Point;

use super::Project;
use crate::lsp_store::FormatTrigger;
use crate::types::*;
use crate::{DocumentHighlight, Hover, InlayHint, LocationLink, PrepareRenameResponse, ProjectPath, ProjectTransaction, ResolvedPath, Symbol};

impl Project {
    pub fn definitions<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<LocationLink>>>> { /* 原样 */ }

    pub fn declarations<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn type_definitions<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn implementations<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn references<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn prepare_call_hierarchy<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn incoming_calls(...) -> ... { /* 原样 */ }
    pub fn outgoing_calls(...) -> ... { /* 原样 */ }
    pub fn document_highlights<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn document_symbols(...) -> ... { /* 原样 */ }
    pub fn symbols(...) -> ... { /* 原样 */ }
    pub fn open_buffer_for_symbol(...) -> ... { /* 原样 */ }
    pub fn hover<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn linked_edits(...) -> ... { /* 原样 */ }
    pub fn completions<T: ToOffset + ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn code_actions<T: Clone + ToOffset>(...) -> ... { /* 原样 */ }
    pub fn apply_code_action(...) -> ... { /* 原样 */ }
    pub fn apply_code_action_kind(...) -> ... { /* 原样 */ }
    pub fn prepare_rename<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn perform_rename<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn on_type_format<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn inline_values(...) -> ... { /* 原样 */ }
    pub fn request_lsp<R: LspCommand>(...) -> ... { /* 原样 */ }
    pub fn format(...) -> ... { /* 原样 */ }
    pub fn supports_range_formatting(...) -> bool { /* 原样 */ }
    pub fn edit_prediction_definitions<T: ToPointUtf16>(...) -> ... { /* 原样 */ }
    pub fn set_language_for_buffer(...) { /* 原样 */ }
    pub fn restart_language_servers_for_buffers(...) { /* 原样 */ }
    pub fn stop_language_servers_for_buffers(...) { /* 原样 */ }
    pub fn cancel_language_server_work_for_buffers(...) { /* 原样 */ }
    pub fn cancel_language_server_work(...) { /* 原样 */ }
    pub fn open_local_buffer_via_lsp(...) -> ... { /* 原样 */ }
    pub fn open_server_settings(...) -> ... { /* 原样 */ }
    pub fn register_buffer_with_language_servers(...) -> OpenLspBufferHandle { /* 原样 */ }
    pub fn language_server_statuses<'a>(...) -> ... { /* 原样 */ }
    pub fn last_formatting_failure<'a>(...) -> ... { /* 原样 */ }
    pub fn reset_last_formatting_failure(...) { /* 原样 */ }
    pub fn any_language_server_supports_inlay_hints(...) -> bool { /* 原样 */ }
    pub fn any_language_server_supports_semantic_tokens(...) -> bool { /* 原样 */ }
    pub fn language_server_id_for_name(...) -> Option<LanguageServerId> { /* 原样 */ }
    #[cfg(feature = "test-support")]
    pub fn has_language_servers_for(...) -> bool { /* 原样 */ }
}
