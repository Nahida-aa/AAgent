use super::*;

use std::path::PathBuf;
use std::sync::Arc;

use clock::ReplicaId;
use language::{Buffer, Capability};
use parking_lot::Mutex;
use worktree::Worktree;

use crate::lsp_store::LanguageServerName;

/// 构造 Project 时的本地化开关（对齐 zed `LocalProjectFlags`）。
#[derive(Clone, Copy, Debug)]
pub struct LocalProjectFlags {
    pub init_worktree_trust: bool,
    pub watch_global_configs: bool,
}

impl Default for LocalProjectFlags {
    fn default() -> Self {
        Self {
            init_worktree_trust: true,
            watch_global_configs: true,
        }
    }
}

/// Message ordered with respect to buffer operations
#[derive(Debug)]
pub(crate) enum BufferOrderedMessage {
    Operation {
        buffer_id: language::BufferId,
        operation: rpc::proto::Operation,
    },
    LanguageServerUpdate {
        language_server_id: language::LanguageServerId,
        message: rpc::proto::update_language_server::Variant,
        name: Option<LanguageServerName>,
    },
    Resync,
}

#[derive(Debug)]
pub(crate) enum ProjectClientState {
    Local,
    Shared { remote_id: u64 },
    Collab {
        sharing_has_stopped: bool,
        capability: Capability,
        remote_id: u64,
        replica_id: ReplicaId,
    },
}

pub struct DownloadingFile {
    pub(crate) destination_path: PathBuf,
    pub(crate) chunks: Vec<u8>,
    pub(crate) total_size: u64,
    pub(crate) file_id: Option<u64>,
}

#[derive(Default)]
pub(crate) struct RemotelyCreatedModels {
    pub(crate) worktrees: Vec<gpui::Entity<Worktree>>,
    pub(crate) buffers: Vec<gpui::Entity<Buffer>>,
    pub(crate) retain_count: usize,
}

pub(crate) struct RemotelyCreatedModelGuard {
    pub(crate) remote_models: std::sync::Weak<Mutex<RemotelyCreatedModels>>,
}

impl Drop for RemotelyCreatedModelGuard {
    fn drop(&mut self) {
        if let Some(remote_models) = self.remote_models.upgrade() {
            let mut remote_models = remote_models.lock();
            assert!(
                remote_models.retain_count > 0,
                "RemotelyCreatedModelGuard dropped too many times"
            );
            remote_models.retain_count -= 1;
            if remote_models.retain_count == 0 {
                remote_models.buffers.clear();
                remote_models.worktrees.clear();
            }
        }
    }
}

pub enum DebugAdapterClientState {
    Starting(gpui::Task<Option<Arc<::dap::client::DebugAdapterClient>>>),
    Running(Arc<::dap::client::DebugAdapterClient>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLocation {
    pub buffer: gpui::WeakEntity<Buffer>,
    pub position: text::Anchor,
}
