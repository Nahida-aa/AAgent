use anyhow::{Context as _, Result};
use client::TypedEnvelope;
use gpui::AsyncApp;
use itertools::Itertools;
use rpc::AnyProtoClient;
use std::collections::HashSet;

use super::Project;
use crate::types::*;
use crate::{Event, LanguageServerPromptRequest, LanguageServerShowDocumentRequest};

impl Project {
    pub(crate) async fn handle_language_server_prompt_request(
        this: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::LanguageServerPromptRequest>,
        mut cx: AsyncApp,
    ) -> Result<proto::LanguageServerPromptResponse> { /* 原样 */ }

    pub(crate) async fn handle_language_server_show_document_request(
        project: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::LanguageServerShowDocumentRequest>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> { /* 原样 */ }

    pub(crate) async fn handle_update_buffer(
        this: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateBuffer>,
        cx: AsyncApp,
    ) -> Result<proto::Ack> { /* 原样 */ }

    pub(crate) async fn handle_update_buffer_from_remote_server(
        this: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateBuffer>,
        cx: AsyncApp,
    ) -> Result<proto::Ack> { /* 原样 */ }

    pub(crate) async fn handle_synchronize_buffers(
        this: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::SynchronizeBuffers>,
        mut cx: AsyncApp,
    ) -> Result<proto::SynchronizeBuffersResponse> { /* 原样 */ }

    pub(crate) async fn handle_toggle_lsp_logs(
        project: gpui::Entity<Self>,
        envelope: TypedEnvelope<proto::ToggleLspLogs>,
        mut cx: AsyncApp,
    ) -> Result<()> { /* 原样 */ }
}
