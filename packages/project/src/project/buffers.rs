use super::*;

use ::rpc::proto;
use std::sync::Arc;
use std::time::Duration;

use crate::buffer_store::BufferStore;

use std::collections::HashSet;

use anyhow::{Context as _, Result, anyhow};
use futures::{
    FutureExt as _, StreamExt as _,
    channel::mpsc::UnboundedReceiver,
    future::try_join_all,
};
use gpui::{App, AsyncApp, Context, Entity, Task, WeakEntity};
use itertools::Either;
use language::{
    Buffer, BufferEvent, BufferId, Capability, DiskState, Rope, ToOffset,
    proto::split_operations,
};
use ::rpc::ErrorCode;

use super::state::BufferOrderedMessage;
use super::Project;
use crate::buffer_store::{BufferStoreEvent, ProjectTransaction};
use crate::project_settings::ProjectSettings;
use language::File;
use crate::ProjectPath;

impl Project {
    pub fn create_buffer(
        &mut self,
        language: Option<Arc<Language>>,
        project_searchable: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> {
        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.create_buffer(language, project_searchable, cx)
        })
    }

    pub fn create_local_buffer(
        &mut self,
        text: &str,
        language: Option<Arc<Language>>,
        project_searchable: bool,
        cx: &mut Context<Self>,
    ) -> Entity<Buffer> {
        if self.is_remote() {
            panic!("called create_local_buffer on a remote project")
        }
        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.create_local_buffer(text, language, project_searchable, cx)
        })
    }

    pub fn open_path(
        &mut self,
        path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Task<Result<(Option<ProjectEntryId>, Entity<Buffer>)>> {
        let task = self.open_buffer(path, cx);
        cx.spawn(async move |_project, cx| {
            let buffer = task.await?;
            let project_entry_id = buffer.read_with(cx, |buffer, _cx| {
                File::from_dyn(buffer.file()).and_then(|file| file.project_entry_id())
            });

            Ok((project_entry_id, buffer))
        })
    }

    pub fn open_local_buffer(
        &mut self,
        abs_path: impl AsRef<Path>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> {
        let worktree_task = self.find_or_create_worktree(abs_path.as_ref(), false, cx);
        cx.spawn(async move |this, cx| {
            let (worktree, relative_path) = worktree_task.await?;
            this.update(cx, |this, cx| {
                this.open_buffer((worktree.read(cx).id(), relative_path), cx)
            })?
            .await
        })
    }

    #[cfg(feature = "test-support")]

    pub fn open_buffer(
        &mut self,
        path: impl Into<ProjectPath>,
        cx: &mut App,
    ) -> Task<Result<Entity<Buffer>>> {
        if self.is_disconnected(cx) {
            return Task::ready(Err(anyhow!(ErrorCode::Disconnected)));
        }

        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.open_buffer(path.into(), cx)
        })
    }

    #[cfg(feature = "test-support")]

    pub fn open_buffer_by_id(
        &mut self,
        id: BufferId,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> {
        if let Some(buffer) = self.buffer_for_id(id, cx) {
            Task::ready(Ok(buffer))
        } else if self.is_local() || self.is_via_remote_server() {
            Task::ready(Err(anyhow!("buffer {id} does not exist")))
        } else if let Some(project_id) = self.remote_id() {
            let request = self.collab_client.request(proto::OpenBufferById {
                project_id,
                id: id.into(),
            });
            cx.spawn(async move |project, cx| {
                let buffer_id = BufferId::new(request.await?.buffer_id)?;
                project
                    .update(cx, |project, cx| {
                        project.buffer_store.update(cx, |buffer_store, cx| {
                            buffer_store.wait_for_remote_buffer(buffer_id, cx)
                        })
                    })?
                    .await
            })
        } else {
            Task::ready(Err(anyhow!("cannot open buffer while disconnected")))
        }
    }

    pub fn save_buffers(
        &self,
        buffers: HashSet<Entity<Buffer>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        cx.spawn(async move |this, cx| {
            let save_tasks = buffers.into_iter().filter_map(|buffer| {
                this.update(cx, |this, cx| this.save_buffer(buffer, cx))
                    .ok()
            });
            try_join_all(save_tasks).await?;
            Ok(())
        })
    }

    pub fn save_buffer(&self, buffer: Entity<Buffer>, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.buffer_store
            .update(cx, |buffer_store, cx| buffer_store.save_buffer(buffer, cx))
    }
    pub fn save_buffer_as(
        &mut self,
        buffer: Entity<Buffer>,
        path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.save_buffer_as(buffer.clone(), path, cx)
        })
    }
    pub fn get_open_buffer(&self, path: &ProjectPath, cx: &App) -> Option<Entity<Buffer>> {
        self.buffer_store.read(cx).get_by_path(path)
    }
    pub fn dirty_buffers<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = ProjectPath> + 'a {
        self.buffer_store.read(cx).buffers().filter_map(|buf| {
            let buf = buf.read(cx);
            if buf.is_dirty() {
                buf.project_path(cx)
            } else {
                None
            }
        })
    }
    pub fn reload_buffers(
        &self,
        buffers: HashSet<Entity<Buffer>>,
        push_to_history: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectTransaction>> {
        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.reload_buffers(buffers, push_to_history, cx)
        })
    }

    fn register_buffer(&mut self, buffer: &Entity<Buffer>, cx: &mut Context<Self>) -> Result<()> {
        {
            let mut remotely_created_models = self.remotely_created_models.lock();
            if remotely_created_models.retain_count > 0 {
                remotely_created_models.buffers.push(buffer.clone())
            }
        }

        self.request_buffer_diff_recalculation(buffer, cx);

        cx.subscribe(buffer, |this, buffer, event, cx| {
            this.on_buffer_event(buffer, event, cx);
        })
        .detach();

        Ok(())
    }

    fn on_buffer_store_event(
        &mut self,
        _: Entity<BufferStore>,
        event: &BufferStoreEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            BufferStoreEvent::BufferAdded(buffer) => {
                self.register_buffer(buffer, cx).log_err();
            }
            BufferStoreEvent::BufferDropped(buffer_id) => {
                if let Some(ref remote_client) = self.remote_client {
                    remote_client
                        .read(cx)
                        .proto_client()
                        .send(proto::CloseBuffer {
                            project_id: 0,
                            buffer_id: buffer_id.to_proto(),
                        })
                        .log_err();
                }
            }
            _ => {}
        }
    }

    fn on_buffer_event(
        &mut self,
        buffer: Entity<Buffer>,
        event: &BufferEvent,
        cx: &mut Context<Self>,
    ) -> Option<()> {
        if matches!(event, BufferEvent::Edited { .. } | BufferEvent::Reloaded) {
            self.request_buffer_diff_recalculation(&buffer, cx);
        }

        if let BufferEvent::Edited { source } = event {
            cx.emit(Event::BufferEdited { source: *source });
        }

        let buffer_id = buffer.read(cx).remote_id();
        match event {
            BufferEvent::ReloadNeeded => {
                if !self.is_via_collab() {
                    self.reload_buffers([buffer.clone()].into_iter().collect(), true, cx)
                        .detach_and_log_err(cx);
                }
            }
            BufferEvent::Operation {
                operation,
                is_local: true,
            } => {
                let operation = language::proto::serialize_operation(operation);

                if let Some(remote) = &self.remote_client {
                    remote
                        .read(cx)
                        .proto_client()
                        .send(proto::UpdateBuffer {
                            project_id: 0,
                            buffer_id: buffer_id.to_proto(),
                            operations: vec![operation.clone()],
                        })
                        .ok();
                }

                self.enqueue_buffer_ordered_message(BufferOrderedMessage::Operation {
                    buffer_id,
                    operation,
                })
                .ok();
            }

            _ => {}
        }

        None
    }

    fn request_buffer_diff_recalculation(
        &mut self,
        buffer: &Entity<Buffer>,
        cx: &mut Context<Self>,
    ) {
        self.buffers_needing_diff.insert(buffer.downgrade());
        let first_insertion = self.buffers_needing_diff.len() == 1;
        let settings = ProjectSettings::get_global(cx);
        let delay = settings.git.gutter_debounce;

        if delay == 0 {
            if first_insertion {
                let this = cx.weak_entity();
                cx.defer(move |cx| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |this, cx| {
                            this.recalculate_buffer_diffs(cx).detach();
                        });
                    }
                });
            }
            return;
        }

        const MIN_DELAY: u64 = 50;
        let delay = delay.max(MIN_DELAY);
        let duration = Duration::from_millis(delay);

        self.git_diff_debouncer
            .fire_new(duration, cx, move |this, cx| {
                this.recalculate_buffer_diffs(cx)
            });
    }

    fn recalculate_buffer_diffs(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let task = this
                    .update(cx, |this, cx| {
                        let buffers = this
                            .buffers_needing_diff
                            .drain()
                            .filter_map(|buffer| buffer.upgrade())
                            .collect::<Vec<_>>();
                        if buffers.is_empty() {
                            None
                        } else {
                            Some(this.git_store.update(cx, |git_store, cx| {
                                git_store.recalculate_buffer_diffs(buffers, cx)
                            }))
                        }
                    })
                    .ok()
                    .flatten();

                if let Some(task) = task {
                    task.await;
                } else {
                    break;
                }
            }
        })
    }

    async fn send_buffer_ordered_messages(
        project: WeakEntity<Self>,
        rx: UnboundedReceiver<BufferOrderedMessage>,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        const MAX_BATCH_SIZE: usize = 128;

        let mut operations_by_buffer_id = HashMap::default();
        async fn flush_operations(
            this: &WeakEntity<Project>,
            operations_by_buffer_id: &mut HashMap<BufferId, Vec<proto::Operation>>,
            needs_resync_with_host: &mut bool,
            is_local: bool,
            cx: &mut AsyncApp,
        ) -> Result<()> {
            for (buffer_id, operations) in operations_by_buffer_id.drain() {
                let request = this.read_with(cx, |this, _| {
                    let project_id = this.remote_id()?;
                    Some(this.collab_client.request(proto::UpdateBuffer {
                        buffer_id: buffer_id.into(),
                        project_id,
                        operations,
                    }))
                })?;
                if let Some(request) = request
                    && request.await.is_err()
                    && !is_local
                {
                    *needs_resync_with_host = true;
                    break;
                }
            }
            Ok(())
        }

        let mut needs_resync_with_host = false;
        let mut changes = rx.ready_chunks(MAX_BATCH_SIZE);

        while let Some(changes) = changes.next().await {
            let is_local = project.read_with(cx, |this, _| this.is_local())?;

            for change in changes {
                match change {
                    BufferOrderedMessage::Operation {
                        buffer_id,
                        operation,
                    } => {
                        if needs_resync_with_host {
                            continue;
                        }

                        operations_by_buffer_id
                            .entry(buffer_id)
                            .or_insert(Vec::new())
                            .push(operation);
                    }

                    BufferOrderedMessage::Resync => {
                        operations_by_buffer_id.clear();
                        if project
                            .update(cx, |this, cx| this.synchronize_remote_buffers(cx))?
                            .await
                            .is_ok()
                        {
                            needs_resync_with_host = false;
                        }
                    }

                    BufferOrderedMessage::LanguageServerUpdate {
                        language_server_id,
                        message,
                        name,
                    } => {
                        flush_operations(
                            &project,
                            &mut operations_by_buffer_id,
                            &mut needs_resync_with_host,
                            is_local,
                            cx,
                        )
                        .await?;

                        project.read_with(cx, |project, _| {
                            if let Some(project_id) = project.remote_id() {
                                project
                                    .collab_client
                                    .send(proto::UpdateLanguageServer {
                                        project_id,
                                        server_name: name.map(|name| String::from(name.0)),
                                        language_server_id: language_server_id.to_proto(),
                                        variant: Some(message),
                                    })
                                    .log_err();
                            }
                        })?;
                    }
                }
            }

            flush_operations(
                &project,
                &mut operations_by_buffer_id,
                &mut needs_resync_with_host,
                is_local,
                cx,
            )
            .await?;
        }

        Ok(())
    }

    fn enqueue_buffer_ordered_message(&mut self, message: BufferOrderedMessage) -> Result<()> {
        self.buffer_ordered_messages_tx
            .unbounded_send(message)
            .map_err(|e| anyhow!(e))
    }

    fn synchronize_remote_buffers(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let project_id = match self.client_state {
            ProjectClientState::Collab {
                sharing_has_stopped,
                remote_id,
                ..
            } => {
                if sharing_has_stopped {
                    return Task::ready(Err(anyhow!(
                        "can't synchronize remote buffers on a readonly project"
                    )));
                } else {
                    remote_id
                }
            }
            ProjectClientState::Shared { .. } | ProjectClientState::Local => {
                return Task::ready(Err(anyhow!(
                    "can't synchronize remote buffers on a local project"
                )));
            }
        };

        let client = self.collab_client.clone();
        cx.spawn(async move |this, cx| {
            let (buffers, incomplete_buffer_ids) = this.update(cx, |this, cx| {
                this.buffer_store.read(cx).buffer_version_info(cx)
            })?;
            let response = client
                .request(proto::SynchronizeBuffers {
                    project_id,
                    buffers,
                })
                .await?;

            let send_updates_for_buffers = this.update(cx, |this, cx| {
                response
                    .buffers
                    .into_iter()
                    .map(|buffer| {
                        let client = client.clone();
                        let buffer_id = match BufferId::new(buffer.id) {
                            Ok(id) => id,
                            Err(e) => {
                                return Task::ready(Err(e));
                            }
                        };
                        let remote_version = language::proto::deserialize_version(&buffer.version);
                        if let Some(buffer) = this.buffer_for_id(buffer_id, cx) {
                            let operations =
                                buffer.read(cx).serialize_ops(Some(remote_version), cx);
                            cx.background_spawn(async move {
                                let operations = operations.await;
                                for chunk in split_operations(operations) {
                                    client
                                        .request(proto::UpdateBuffer {
                                            project_id,
                                            buffer_id: buffer_id.into(),
                                            operations: chunk,
                                        })
                                        .await?;
                                }
                                anyhow::Ok(())
                            })
                        } else {
                            Task::ready(Ok(()))
                        }
                    })
                    .collect::<Vec<_>>()
            })?;

            // Any incomplete buffers have open requests waiting. Request that the host sends
            // creates these buffers for us again to unblock any waiting futures.
            for id in incomplete_buffer_ids {
                cx.background_spawn(client.request(proto::OpenBufferById {
                    project_id,
                    id: id.into(),
                }))
                .detach();
            }

            futures::future::join_all(send_updates_for_buffers)
                .await
                .into_iter()
                .collect()
        })
    }
}
