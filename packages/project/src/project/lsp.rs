use super::*;

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use anyhow::Result;
use anyhow::anyhow;
use gpui::{App, AppContext, Context, Entity, Task, TaskExt};
use language::{
    Anchor, Buffer, Language, Location, PointUtf16, ToOffset, ToPointUtf16, Transaction, Unclipped,
};
use lsp::{
    CodeActionKind, CompletionContext, LanguageServerId, LanguageServerName, LanguageServerSelector,
};
use rpc::proto::{self, REMOTE_SERVER_PROJECT_ID};
use text::{BufferId, Point};

use crate::debugger::{breakpoint_store::ActiveStackFrame, session::Session};
use crate::lsp_command::*;
use crate::lsp_store::{
    CompletionDocumentation, FormatTrigger, LanguageServerStatus, LspFormatTarget,
    OpenLspBufferHandle,
};
use crate::protocol_helpers::provide_inline_values;
use crate::types::*;
use crate::{
    DocumentHighlight, Hover, InlayHint, LocationLink, PrepareRenameResponse, ProjectPath,
    ProjectTransaction, ResolvedPath, Symbol,
};

use super::Project;

impl Project {
    pub fn register_buffer_with_language_servers(
        &self,
        buffer: &Entity<Buffer>,
        cx: &mut App,
    ) -> OpenLspBufferHandle {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.register_buffer_with_language_servers(buffer, HashSet::default(), false, cx)
        })
    }

    pub fn set_language_for_buffer(
        &mut self,
        buffer: &Entity<Buffer>,
        new_language: Arc<Language>,
        cx: &mut Context<Self>,
    ) {
        buffer.update(cx, |buffer, _| {
            buffer.set_content_language_detection_enabled(false);
        });
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.set_language_for_buffer(buffer, new_language, cx)
        })
    }

    pub fn restart_language_servers_for_buffers(
        &mut self,
        buffers: Vec<Entity<Buffer>>,
        only_restart_servers: HashSet<LanguageServerSelector>,
        clear_stopped: bool,
        cx: &mut Context<Self>,
    ) {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.restart_language_servers_for_buffers(
                buffers,
                only_restart_servers,
                clear_stopped,
                cx,
            )
        })
    }

    pub fn stop_language_servers_for_buffers(
        &mut self,
        buffers: Vec<Entity<Buffer>>,
        also_restart_servers: HashSet<LanguageServerSelector>,
        cx: &mut Context<Self>,
    ) {
        self.lsp_store
            .update(cx, |lsp_store, cx| {
                lsp_store.stop_language_servers_for_buffers(buffers, also_restart_servers, cx)
            })
            .detach_and_log_err(cx);
    }

    pub fn cancel_language_server_work_for_buffers(
        &mut self,
        buffers: impl IntoIterator<Item = Entity<Buffer>>,
        cx: &mut Context<Self>,
    ) {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.cancel_language_server_work_for_buffers(buffers, cx)
        })
    }

    pub fn cancel_language_server_work(
        &mut self,
        server_id: LanguageServerId,
        token_to_cancel: Option<ProgressToken>,
        cx: &mut Context<Self>,
    ) {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.cancel_language_server_work(server_id, token_to_cancel, cx)
        })
    }

    pub fn language_server_statuses<'a>(
        &'a self,
        cx: &'a App,
    ) -> impl DoubleEndedIterator<Item = (LanguageServerId, &'a LanguageServerStatus)> {
        self.lsp_store.read(cx).language_server_statuses()
    }

    pub fn last_formatting_failure<'a>(&self, cx: &'a App) -> Option<&'a str> {
        self.lsp_store.read(cx).last_formatting_failure()
    }

    pub fn reset_last_formatting_failure(&self, cx: &mut App) {
        self.lsp_store
            .update(cx, |store, _| store.reset_last_formatting_failure());
    }

    pub fn format(
        &mut self,
        buffers: HashSet<Entity<Buffer>>,
        target: LspFormatTarget,
        push_to_history: bool,
        trigger: FormatTrigger,
        cx: &mut Context<Project>,
    ) -> Task<anyhow::Result<ProjectTransaction>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.format(buffers, target, push_to_history, trigger, cx)
        })
    }

    pub fn supports_range_formatting(&self, buffer: &Entity<Buffer>, cx: &App) -> bool {
        self.lsp_store
            .read(cx)
            .supports_range_formatting(buffer, cx)
    }

    pub fn definitions<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<LocationLink>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.definitions(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn edit_prediction_definitions<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        include_type_definitions: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<EditPredictionDefinition>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.edit_prediction_definitions(buffer, position, include_type_definitions, cx)
        })
    }

    pub fn declarations<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<LocationLink>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.declarations(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn type_definitions<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<LocationLink>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.type_definitions(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn implementations<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<LocationLink>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.implementations(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn references<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<Location>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.references(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn prepare_call_hierarchy<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<CallHierarchyItem>>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.prepare_call_hierarchy(buffer, position, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn incoming_calls(
        &mut self,
        item: CallHierarchyItem,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<IncomingCall>>>> {
        let guard = self.retain_remotely_created_models(cx);
        let task = self
            .lsp_store
            .update(cx, |lsp_store, cx| lsp_store.incoming_calls(item, cx));
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn outgoing_calls(
        &mut self,
        item: CallHierarchyItem,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<OutgoingCall>>>> {
        let guard = self.retain_remotely_created_models(cx);
        let task = self
            .lsp_store
            .update(cx, |lsp_store, cx| lsp_store.outgoing_calls(item, cx));
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }

    pub fn document_highlights<T: ToPointUtf16>(
        &mut self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<DocumentHighlight>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        self.request_lsp(
            buffer.clone(),
            LanguageServerToQuery::FirstCapable,
            GetDocumentHighlights { position },
            cx,
        )
    }

    pub fn document_symbols(
        &mut self,
        buffer: &Entity<Buffer>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<DocumentSymbol>>> {
        self.request_lsp(
            buffer.clone(),
            LanguageServerToQuery::FirstCapable,
            GetDocumentSymbols,
            cx,
        )
    }

    pub fn symbols(&self, query: &str, cx: &mut Context<Self>) -> Task<Result<Vec<Symbol>>> {
        self.lsp_store
            .update(cx, |lsp_store, cx| lsp_store.symbols(query, cx))
    }

    pub fn open_buffer_for_symbol(
        &mut self,
        symbol: &Symbol,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.open_buffer_for_symbol(symbol, cx)
        })
    }

    pub fn open_server_settings(&mut self, cx: &mut Context<Self>) -> Task<Result<Entity<Buffer>>> {
        let guard = self.retain_remotely_created_models(cx);
        let Some(remote) = self.remote_client.as_ref() else {
            return Task::ready(Err(anyhow!("not an ssh project")));
        };

        let proto_client = remote.read(cx).proto_client();

        cx.spawn(async move |project, cx| {
            let buffer = proto_client
                .request(proto::OpenServerSettings {
                    project_id: REMOTE_SERVER_PROJECT_ID,
                })
                .await?;

            let buffer = project
                .update(cx, |project, cx| {
                    project.buffer_store.update(cx, |buffer_store, cx| {
                        anyhow::Ok(
                            buffer_store
                                .wait_for_remote_buffer(BufferId::new(buffer.buffer_id)?, cx),
                        )
                    })
                })??
                .await;

            drop(guard);
            buffer
        })
    }

    pub fn open_local_buffer_via_lsp(
        &mut self,
        abs_path: ::lsp::Uri,
        language_server_id: LanguageServerId,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.open_local_buffer_via_lsp(abs_path, language_server_id, cx)
        })
    }

    pub fn hover<T: ToPointUtf16>(
        &self,
        buffer: &Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Option<Vec<Hover>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        self.lsp_store
            .update(cx, |lsp_store, cx| lsp_store.hover(buffer, position, cx))
    }

    pub fn linked_edits(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<Range<Anchor>>>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.linked_edits(buffer, position, cx)
        })
    }

    pub fn completions<T: ToOffset + ToPointUtf16>(
        &self,
        buffer: &Entity<Buffer>,
        position: T,
        context: CompletionContext,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<CompletionResponse>>> {
        let position = position.to_point_utf16(buffer.read(cx));
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.completions(buffer, position, context, cx)
        })
    }

    pub fn code_actions<T: Clone + ToOffset>(
        &mut self,
        buffer_handle: &Entity<Buffer>,
        range: Range<T>,
        kinds: Option<Vec<CodeActionKind>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Vec<CodeAction>>>> {
        let buffer = buffer_handle.read(cx);
        let range = buffer.anchor_before(range.start)..buffer.anchor_before(range.end);
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.code_actions(buffer_handle, range, kinds, cx)
        })
    }

    pub fn apply_code_action(
        &self,
        buffer_handle: Entity<Buffer>,
        action: CodeAction,
        push_to_history: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectTransaction>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.apply_code_action(buffer_handle, action, push_to_history, cx)
        })
    }

    pub fn apply_code_action_kind(
        &self,
        buffers: HashSet<Entity<Buffer>>,
        kind: CodeActionKind,
        push_to_history: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectTransaction>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.apply_code_action_kind(buffers, kind, push_to_history, cx)
        })
    }

    pub fn prepare_rename<T: ToPointUtf16>(
        &mut self,
        buffer: Entity<Buffer>,
        position: T,
        cx: &mut Context<Self>,
    ) -> Task<Result<PrepareRenameResponse>> {
        let position = position.to_point_utf16(buffer.read(cx));
        self.request_lsp(
            buffer,
            LanguageServerToQuery::FirstCapable,
            PrepareRename { position },
            cx,
        )
    }

    pub fn perform_rename<T: ToPointUtf16>(
        &mut self,
        buffer: Entity<Buffer>,
        position: T,
        new_name: String,
        language_server_id: Option<LanguageServerId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectTransaction>> {
        let push_to_history = true;
        let position = position.to_point_utf16(buffer.read(cx));
        let mut request = PerformRename {
            position,
            new_name,
            push_to_history,
            language_server_id,
        };
        if let Some(server_id) = request.language_server_id {
            let server_is_capable = !self.is_local()
                || self.lsp_store.update(cx, |lsp_store, cx| {
                    lsp_store
                        .language_server_capable_of_lsp_request(&buffer, server_id, &request, cx)
                });
            if !server_is_capable {
                request.language_server_id = None;
            }
        }
        let server_to_query = request
            .language_server_id
            .map(LanguageServerToQuery::Other)
            .unwrap_or(LanguageServerToQuery::FirstCapable);
        self.request_lsp(buffer, server_to_query, request, cx)
    }

    pub fn on_type_format<T: ToPointUtf16>(
        &mut self,
        buffer: Entity<Buffer>,
        position: T,
        trigger: String,
        push_to_history: bool,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<Option<Transaction>>>> {
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.on_type_format(buffer, position, trigger, push_to_history, cx)
        })
    }

    pub fn inline_values(
        &mut self,
        session: Entity<Session>,
        active_stack_frame: ActiveStackFrame,
        buffer_handle: Entity<Buffer>,
        range: Range<Anchor>,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Vec<InlayHint>>> {
        let snapshot = buffer_handle.read(cx).snapshot();

        let captures =
            snapshot.debug_variables_query(Anchor::min_for_buffer(snapshot.remote_id())..range.end);

        let row = snapshot
            .summary_for_anchor::<PointUtf16>(&range.end)
            .row as usize;

        let inline_value_locations = provide_inline_values(captures, &snapshot, row);

        let stack_frame_id = active_stack_frame.stack_frame_id;
        cx.spawn(async move |this, cx| {
            this.update(cx, |project, cx| {
                project.dap_store().update(cx, |dap_store, cx| {
                    dap_store.resolve_inline_value_locations(
                        session,
                        stack_frame_id,
                        buffer_handle,
                        inline_value_locations,
                        cx,
                    )
                })
            })?
            .await
        })
    }

    pub fn request_lsp<R: LspCommand>(
        &mut self,
        buffer_handle: Entity<Buffer>,
        server: LanguageServerToQuery,
        request: R,
        cx: &mut Context<Self>,
    ) -> Task<Result<R::Response>>
    where
        <R::LspRequest as ::lsp::request::Request>::Result: Send,
        <R::LspRequest as ::lsp::request::Request>::Params: Send,
    {
        let guard = self.retain_remotely_created_models(cx);
        let task = self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.request_lsp(buffer_handle, server, request, cx)
        });
        cx.background_spawn(async move {
            let result = task.await;
            drop(guard);
            result
        })
    }
    pub fn any_language_server_supports_inlay_hints(&self, buffer: &Buffer, cx: &mut App) -> bool {
        let Some(language) = buffer.language().cloned() else {
            return false;
        };
        self.lsp_store.update(cx, |lsp_store, _| {
            let relevant_language_servers = lsp_store
                .languages
                .lsp_adapters(&language.name())
                .into_iter()
                .map(|lsp_adapter| lsp_adapter.name())
                .collect::<HashSet<_>>();
            lsp_store
                .language_server_statuses()
                .filter_map(|(server_id, server_status)| {
                    relevant_language_servers
                        .contains(&server_status.name)
                        .then_some(server_id)
                })
                .filter_map(|server_id| lsp_store.lsp_server_capabilities.get(&server_id))
                .any(InlayHints::check_capabilities)
        })
    }

    pub fn any_language_server_supports_semantic_tokens(
        &self,
        buffer: &Buffer,
        cx: &mut App,
    ) -> bool {
        let Some(language) = buffer.language().cloned() else {
            return false;
        };
        let lsp_store = self.lsp_store.read(cx);
        let relevant_language_servers = lsp_store
            .languages
            .lsp_adapters(&language.name())
            .into_iter()
            .map(|lsp_adapter| lsp_adapter.name())
            .collect::<HashSet<_>>();
        lsp_store
            .language_server_statuses()
            .filter_map(|(server_id, server_status)| {
                relevant_language_servers
                    .contains(&server_status.name)
                    .then_some(server_id)
            })
            .filter_map(|server_id| lsp_store.lsp_server_capabilities.get(&server_id))
            .any(|capabilities| capabilities.semantic_tokens_provider.is_some())
    }

    pub fn language_server_id_for_name(
        &self,
        buffer: &Buffer,
        name: &LanguageServerName,
        cx: &App,
    ) -> Option<LanguageServerId> {
        let language = buffer.language()?;
        let relevant_language_servers = self
            .languages
            .lsp_adapters(&language.name())
            .into_iter()
            .map(|lsp_adapter| lsp_adapter.name())
            .collect::<HashSet<_>>();
        if !relevant_language_servers.contains(name) {
            return None;
        }
        let opened_in_servers = self
            .lsp_store
            .read(cx)
            .language_server_ids_for_opened_buffer(buffer.remote_id());
        self.language_server_statuses(cx)
            .filter(|(_, server_status)| relevant_language_servers.contains(&server_status.name))
            .find_map(|(server_id, server_status)| {
                (&server_status.name == name
                    && opened_in_servers.is_none_or(|server_ids| server_ids.contains(&server_id)))
                .then_some(server_id)
            })
    }

    #[cfg(feature = "test-support")]
    pub fn has_language_servers_for(&self, buffer: &Buffer, cx: &mut App) -> bool {
        self.lsp_store.update(cx, |this, cx| {
            this.running_language_servers_for_local_buffer(buffer, cx)
                .next()
                .is_some()
        })
    }

}
