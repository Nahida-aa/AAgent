use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Entity, SharedString};
use language::{Buffer, BufferEditSource, BufferId};
use lsp::{LanguageServerId, LanguageServerName};
use rpc::proto;
use snippet::Snippet;
use worktree::{ProjectEntryId, UpdatedEntriesSet, WorktreeId};

use crate::lsp_store::log_store::LogKind;
use crate::lsp_store::{LanguageServerLogType, LanguageServerPromptRequest,
                       LanguageServerShowDocumentRequest};
use crate::toast::ToastLink;
use crate::worktree_store::WorktreePaths;
use crate::{ProjectPath, ProjectTransaction};

pub enum OpenedBufferEvent {
    Disconnected,
    Ok(BufferId),
    Err(BufferId, Arc<anyhow::Error>),
}

/// Semantics-aware entity that is relevant to one or more [`Worktree`] with the files.
/// `Project` is responsible for tasks, LSP and collab queries, synchronizing worktree states accordingly.
/// Maps [`Worktree`] entries with its own logic using [`ProjectEntryId`] and [`ProjectPath`] structs.
///
/// Can be either local (for the project opened on the same host) or remote.(for collab projects, browsed by multiple remote users).

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    LanguageServerAdded(LanguageServerId, LanguageServerName, Option<WorktreeId>),
    SupplementaryLanguageServerAdded(LanguageServerId, LanguageServerName),
    LanguageServerRemoved(LanguageServerId),
    SupplementaryLanguageServerRemoved(LanguageServerId),
    LanguageServerLog(LanguageServerId, LanguageServerLogType, String),
    // [`lsp::notification::DidOpenTextDocument`] was sent to this server using the buffer data.
    // Zed's buffer-related data is updated accordingly.
    LanguageServerBufferRegistered {
        server_id: LanguageServerId,
        buffer_id: BufferId,
        buffer_abs_path: PathBuf,
        name: Option<LanguageServerName>,
    },
    ToggleLspLogs {
        server_id: LanguageServerId,
        enabled: bool,
        toggled_log_kind: LogKind,
    },
    Toast {
        notification_id: SharedString,
        message: String,
        /// Optional link to display as a button in the toast.
        link: Option<ToastLink>,
    },
    HideToast {
        notification_id: SharedString,
    },
    LanguageServerPrompt(LanguageServerPromptRequest),
    LanguageServerShowDocument(LanguageServerShowDocumentRequest),
    LanguageNotFound(Entity<Buffer>),
    ActiveEntryChanged(Option<ProjectEntryId>),
    ActivateProjectPanel,
    WorktreeAdded(WorktreeId),
    WorktreeOrderChanged,
    WorktreeRemoved(WorktreeId),
    WorktreeUpdatedEntries(WorktreeId, UpdatedEntriesSet),
    WorktreeUpdatedRootRepoCommonDir(WorktreeId),
    WorktreePathsChanged {
        old_worktree_paths: WorktreePaths,
    },
    DiskBasedDiagnosticsStarted {
        language_server_id: LanguageServerId,
    },
    DiskBasedDiagnosticsFinished {
        language_server_id: LanguageServerId,
    },
    DiagnosticsUpdated {
        paths: Vec<ProjectPath>,
        language_server_id: LanguageServerId,
    },
    RemoteIdChanged(Option<u64>),
    DisconnectedFromHost,
    DisconnectedFromRemote {
        server_not_running: bool,
    },
    Closed,
    DeletedEntry(WorktreeId, ProjectEntryId),
    CollaboratorUpdated {
        old_peer_id: proto::PeerId,
        new_peer_id: proto::PeerId,
    },
    CollaboratorJoined(proto::PeerId),
    CollaboratorLeft(proto::PeerId),
    HostReshared,
    Reshared,
    Rejoined,
    RefreshInlayHints {
        server_id: LanguageServerId,
    },
    RefreshSemanticTokens {
        server_id: LanguageServerId,
    },
    RefreshCodeLens {
        server_id: Option<LanguageServerId>,
    },
    RefreshDocumentColors {
        server_id: Option<LanguageServerId>,
    },
    RefreshDocumentLinks {
        server_id: Option<LanguageServerId>,
    },
    RefreshDocumentHighlights {
        server_id: Option<LanguageServerId>,
    },
    RefreshFoldingRanges {
        server_id: Option<LanguageServerId>,
    },
    RefreshDocumentSymbols {
        server_id: Option<LanguageServerId>,
    },
    RevealInProjectPanel(ProjectEntryId),
    SnippetEdit(BufferId, Vec<(lsp::Range, Snippet)>),
    ExpandedAllForEntry(WorktreeId, ProjectEntryId),
    EntryRenamed {
        transaction: ProjectTransaction,
        new_project_path: ProjectPath,
        old_abs_path: PathBuf,
        new_abs_path: PathBuf,
    },
    WorkspaceEditApplied(ProjectTransaction),
    AgentLocationChanged,
    BufferEdited {
        source: BufferEditSource,
    },
}

pub struct AgentLocationChanged;
