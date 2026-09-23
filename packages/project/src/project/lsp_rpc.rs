use super::*;

use ::lsp::MessageActionItem;
use ::rpc::AnyProtoClient;
use ::rpc::proto::{self, LanguageServerPromptResponse, REMOTE_SERVER_PROJECT_ID};
use anyhow::{Context as _, Result};
use client::TypedEnvelope;
use futures::StreamExt as _;
use gpui::{AppContext as _, AsyncApp, TaskExt as _};
use itertools::Itertools;
use std::collections::HashSet;
use std::pin::pin;
use std::str::FromStr as _;

use super::Project;
use crate::lsp_store::log_store::LogKind;
use crate::protocol_helpers::proto_to_prompt;
use crate::types::*;
use crate::{Event, LanguageServerPromptRequest, LanguageServerShowDocumentRequest};

impl Project {
    pub(crate) async fn handle_language_server_prompt_request(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::LanguageServerPromptRequest>,
        mut cx: AsyncApp,
    ) -> Result<proto::LanguageServerPromptResponse> {
        let (tx, rx) = async_channel::bounded(1);
        let actions: Vec<_> = envelope
            .payload
            .actions
            .into_iter()
            .map(|action| MessageActionItem {
                title: action,
                properties: Default::default(),
            })
            .collect();
        this.update(&mut cx, |_, cx| {
            cx.emit(Event::LanguageServerPrompt(
                LanguageServerPromptRequest::new(
                    proto_to_prompt(envelope.payload.level.context("Invalid prompt level")?),
                    envelope.payload.message,
                    actions.clone(),
                    envelope.payload.lsp_name,
                    tx,
                ),
            ));

            anyhow::Ok(())
        })?;

        // We drop `this` to avoid holding a reference in this future for too
        // long.
        // If we keep the reference, we might not drop the `Project` early
        // enough when closing a window and it will only get releases on the
        // next `flush_effects()` call.
        drop(this);

        let mut rx = pin!(rx);
        let answer = rx.next().await;

        Ok(LanguageServerPromptResponse {
            action_response: answer.and_then(|answer| {
                actions
                    .iter()
                    .position(|action| *action == answer)
                    .map(|index| index as u64)
            }),
        })
    }

    pub(crate) async fn handle_language_server_show_document_request(
        project: Entity<Self>,
        envelope: TypedEnvelope<proto::LanguageServerShowDocumentRequest>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> {
        let payload = envelope.payload;
        let selection = payload.selection_start.zip(payload.selection_end).map(
            |(selection_start, selection_end)| ::lsp::Range {
                start: ::lsp::Position {
                    line: selection_start.row,
                    character: selection_start.column,
                },
                end: ::lsp::Position {
                    line: selection_end.row,
                    character: selection_end.column,
                },
            },
        );
        let uri = ::lsp::Uri::from_str(&payload.uri)
            .with_context(|| format!("parsing show document uri {}", payload.uri))?;
        let (tx, rx) = async_channel::bounded(1);
        project.update(&mut cx, |_, cx| {
            cx.emit(Event::LanguageServerShowDocument(
                LanguageServerShowDocumentRequest {
                    uri,
                    external: payload.external,
                    take_focus: payload.take_focus,
                    selection,
                    response_channel: tx,
                },
            ));
        });
        drop(project);

        let success = rx.recv().await.unwrap_or(false);
        anyhow::ensure!(
            success,
            "show document request for {} was not handled successfully",
            payload.uri
        );
        Ok(proto::Ack {})
    }

    async fn handle_update_buffer(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateBuffer>,
        cx: AsyncApp,
    ) -> Result<proto::Ack> {
        let buffer_store = this.read_with(&cx, |this, cx| {
            if let Some(ssh) = &this.remote_client {
                let mut payload = envelope.payload.clone();
                payload.project_id = REMOTE_SERVER_PROJECT_ID;
                cx.background_spawn(ssh.read(cx).proto_client().request(payload))
                    .detach_and_log_err(cx);
            }
            this.buffer_store.clone()
        });
        BufferStore::handle_update_buffer(buffer_store, envelope, cx).await
    }

    pub(crate) async fn handle_update_buffer_from_remote_server(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateBuffer>,
        cx: AsyncApp,
    ) -> Result<proto::Ack> {
        let buffer_store = this.read_with(&cx, |this, cx| {
            if let Some(remote_id) = this.remote_id() {
                let mut payload = envelope.payload.clone();
                payload.project_id = remote_id;
                cx.background_spawn(this.collab_client.request(payload))
                    .detach_and_log_err(cx);
            }
            this.buffer_store.clone()
        });
        BufferStore::handle_update_buffer(buffer_store, envelope, cx).await
    }

    async fn handle_synchronize_buffers(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::SynchronizeBuffers>,
        mut cx: AsyncApp,
    ) -> Result<proto::SynchronizeBuffersResponse> {
        let response = this.update(&mut cx, |this, cx| {
            let client = this.collab_client.clone();
            this.buffer_store.update(cx, |this, cx| {
                this.handle_synchronize_buffers(envelope, cx, client)
            })
        })?;

        Ok(response)
    }

    // Goes from client to host.

    async fn handle_toggle_lsp_logs(
        project: Entity<Self>,
        envelope: TypedEnvelope<proto::ToggleLspLogs>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let toggled_log_kind =
            match proto::toggle_lsp_logs::LogType::try_from(envelope.payload.log_type)
                .ok()
                .context("invalid log type")?
            {
                proto::toggle_lsp_logs::LogType::Log => LogKind::Logs,
                proto::toggle_lsp_logs::LogType::Trace => LogKind::Trace,
                proto::toggle_lsp_logs::LogType::Rpc => LogKind::Rpc,
            };
        project.update(&mut cx, |_, cx| {
            cx.emit(Event::ToggleLspLogs {
                server_id: LanguageServerId::from_proto(envelope.payload.server_id),
                enabled: envelope.payload.enabled,
                toggled_log_kind,
            })
        });
        Ok(())
    }
}
