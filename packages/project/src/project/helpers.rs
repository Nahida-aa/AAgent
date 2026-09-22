use std::sync::Arc;

use gpui::{App, AsyncApp, Context, Entity};

use super::state::{RemotelyCreatedModelGuard, RemotelyCreatedModels};
use super::Project;
use crate::buffer_store::BufferStore;
use crate::worktree_store::WorktreeStore;

impl Project {
    pub(crate) fn retain_remotely_created_models(&mut self, cx: &mut Context<Self>) -> RemotelyCreatedModelGuard { /* 原样 */ }

    pub(crate) fn retain_remotely_created_models_impl(
        models: &Arc<parking_lot::Mutex<RemotelyCreatedModels>>,
        buffer_store: &Entity<BufferStore>,
        worktree_store: &Entity<WorktreeStore>,
        cx: &mut App,
    ) -> RemotelyCreatedModelGuard { /* 原样 */ }

    pub(crate) fn respond_to_open_buffer_request(
        this: Entity<Self>,
        buffer: Entity<language::Buffer>,
        peer_id: proto::PeerId,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<proto::OpenBufferResponse> { /* 原样 */ }

    pub(crate) fn create_buffer_for_peer(
        &mut self,
        buffer: &Entity<language::Buffer>,
        peer_id: proto::PeerId,
        cx: &mut App,
    ) -> language::BufferId { /* 原样 */ }
}
