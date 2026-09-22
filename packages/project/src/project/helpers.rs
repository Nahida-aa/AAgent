use super::*;

use std::sync::Arc;

use gpui::{App, AsyncApp, Context, Entity};

use super::state::{RemotelyCreatedModelGuard, RemotelyCreatedModels};
use super::Project;
use crate::buffer_store::BufferStore;
use crate::worktree_store::WorktreeStore;

impl Project {
    pub(crate) fn retain_remotely_created_models(
        &mut self,
        cx: &mut Context<Self>,
    ) -> RemotelyCreatedModelGuard {
        Self::retain_remotely_created_models_impl(
            &self.remotely_created_models,
            &self.buffer_store,
            &self.worktree_store,
            cx,
        )
    }

    pub(crate) fn retain_remotely_created_models_impl(
        models: &Arc<parking_lot::Mutex<RemotelyCreatedModels>>,
        buffer_store: &Entity<BufferStore>,
        worktree_store: &Entity<WorktreeStore>,
        cx: &mut App,
    ) -> RemotelyCreatedModelGuard {
        {
            let mut remotely_create_models = models.lock();
            if remotely_create_models.retain_count == 0 {
                remotely_create_models.buffers = buffer_store.read(cx).buffers().collect();
                remotely_create_models.worktrees = worktree_store.read(cx).worktrees().collect();
            }
            remotely_create_models.retain_count += 1;
        }
        RemotelyCreatedModelGuard {
            remote_models: Arc::downgrade(&models),
        }
    }
}
