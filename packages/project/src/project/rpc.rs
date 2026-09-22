use super::*;

use std::sync::Arc;

use anyhow::{Context as _, Result};
use client::TypedEnvelope;
use futures::StreamExt;
use gpui::{AppContext as _, AsyncApp, Entity, WeakEntity};
use itertools::Itertools;
use rpc::proto;
use util::ResultExt as _;

use super::Project;
use crate::buffer_store::BufferStore;
use crate::lsp_store::CompletionDocumentation;
use crate::crate::project_search::SearchResultsHandle;
use crate::worktree_store::WorktreeStore;
use crate::{Event, ProjectPath};

impl Project {
    async fn handle_unshare_project(
        this: Entity<Self>,
        _: TypedEnvelope<proto::UnshareProject>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |this, cx| {
            if this.is_local() || this.is_via_remote_server() {
                this.unshare(cx)?;
            } else {
                this.disconnected_from_host(cx);
            }
            Ok(())
        })
    }

    async fn handle_add_collaborator(
        this: Entity<Self>,
        mut envelope: TypedEnvelope<proto::AddProjectCollaborator>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let collaborator = envelope
            .payload
            .collaborator
            .take()
            .context("empty collaborator")?;

        let collaborator = Collaborator::from_proto(collaborator)?;
        this.update(&mut cx, |this, cx| {
            this.buffer_store.update(cx, |buffer_store, _| {
                buffer_store.forget_shared_buffers_for(&collaborator.peer_id);
            });
            this.breakpoint_store.read(cx).broadcast();
            cx.emit(Event::CollaboratorJoined(collaborator.peer_id));
            this.collaborators
                .insert(collaborator.peer_id, collaborator);
        });

        Ok(())
    }

    async fn handle_update_project_collaborator(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateProjectCollaborator>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let old_peer_id = envelope
            .payload
            .old_peer_id
            .context("missing old peer id")?;
        let new_peer_id = envelope
            .payload
            .new_peer_id
            .context("missing new peer id")?;
        this.update(&mut cx, |this, cx| {
            let collaborator = this
                .collaborators
                .remove(&old_peer_id)
                .context("received UpdateProjectCollaborator for unknown peer")?;
            let is_host = collaborator.is_host;
            this.collaborators.insert(new_peer_id, collaborator);

            log::info!("peer {} became {}", old_peer_id, new_peer_id,);
            this.buffer_store.update(cx, |buffer_store, _| {
                buffer_store.update_peer_id(&old_peer_id, new_peer_id)
            });

            if is_host {
                this.buffer_store
                    .update(cx, |buffer_store, _| buffer_store.discard_incomplete());
                this.enqueue_buffer_ordered_message(BufferOrderedMessage::Resync)
                    .unwrap();
                cx.emit(Event::HostReshared);
            }

            cx.emit(Event::CollaboratorUpdated {
                old_peer_id,
                new_peer_id,
            });
            Ok(())
        })
    }

    async fn handle_remove_collaborator(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::RemoveProjectCollaborator>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |this, cx| {
            let peer_id = envelope.payload.peer_id.context("invalid peer id")?;
            let replica_id = this
                .collaborators
                .remove(&peer_id)
                .with_context(|| format!("unknown peer {peer_id:?}"))?
                .replica_id;
            this.buffer_store.update(cx, |buffer_store, cx| {
                buffer_store.forget_shared_buffers_for(&peer_id);
                for buffer in buffer_store.buffers() {
                    buffer.update(cx, |buffer, cx| buffer.remove_peer(replica_id, cx));
                }
            });
            this.git_store.update(cx, |git_store, _| {
                git_store.forget_shared_diffs_for(&peer_id);
            });

            cx.emit(Event::CollaboratorLeft(peer_id));
            Ok(())
        })
    }

    async fn handle_update_project(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateProject>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |this, cx| {
            // Don't handle messages that were sent before the response to us joining the project
            if envelope.message_id > this.join_project_response_message_id {
                cx.update_global::<SettingsStore, _>(|store, cx| {
                    for worktree_metadata in &envelope.payload.worktrees {
                        store
                            .clear_local_settings(WorktreeId::from_proto(worktree_metadata.id), cx)
                            .log_err();
                    }
                });

                this.set_worktrees_from_proto(envelope.payload.worktrees, cx)?;
            }
            Ok(())
        })
    }

    async fn handle_toast(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::Toast>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |_, cx| {
            cx.emit(Event::Toast {
                notification_id: envelope.payload.notification_id.into(),
                message: envelope.payload.message,
                link: None,
            });
            Ok(())
        })
    }

    async fn handle_telemetry_event(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::TelemetryEvent>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let payload = envelope.payload;
        this.update(&mut cx, |this, cx| {
            // The remote connection type, OS, version, and architecture are all
            // already known from connection setup, so they don't need to be sent
            // with each event.
            let Some((connection_type, platform, os_version)) =
                this.remote_client.as_ref().map(|client| {
                    let client = client.read(cx);
                    (
                        client.connection_type(),
                        client.remote_platform(),
                        client.remote_os_version(),
                    )
                })
            else {
                return;
            };
            this.client()
                .telemetry()
                .report_remote_event(
                    &payload.event_json,
                    connection_type,
                    platform.os.display_name().to_string(),
                    os_version,
                    platform.arch.as_str().to_string(),
                )
                .log_err();
        });
        Ok(())
    }

    async fn handle_hide_toast(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::HideToast>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |_, cx| {
            cx.emit(Event::HideToast {
                notification_id: envelope.payload.notification_id.into(),
            });
            Ok(())
        })
    }

    // Collab sends UpdateWorktree protos as messages
    async fn handle_update_worktree(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateWorktree>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |project, cx| {
            let worktree_id = WorktreeId::from_proto(envelope.payload.worktree_id);
            if let Some(worktree) = project.worktree_for_id(worktree_id, cx) {
                worktree.update(cx, |worktree, _| {
                    let worktree = worktree.as_remote_mut().unwrap();
                    worktree.update_from_remote(envelope.payload);
                });
            }
            Ok(())
        })
    }

    async fn handle_trust_worktrees(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::TrustWorktrees>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> {
        if this.read_with(&cx, |project, _| project.is_via_collab()) {
            return Ok(proto::Ack {});
        }

        let trusted_worktrees = cx
            .update(|cx| TrustedWorktrees::try_get_global(cx))
            .context("missing trusted worktrees")?;
        trusted_worktrees.update(&mut cx, |trusted_worktrees, cx| {
            trusted_worktrees.trust(
                &this.read(cx).worktree_store(),
                envelope
                    .payload
                    .trusted_paths
                    .into_iter()
                    .filter_map(|proto_path| PathTrust::from_proto(proto_path))
                    .collect(),
                cx,
            );
        });
        Ok(proto::Ack {})
    }

    async fn handle_restrict_worktrees(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::RestrictWorktrees>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> {
        if this.read_with(&cx, |project, _| project.is_via_collab()) {
            return Ok(proto::Ack {});
        }

        let trusted_worktrees = cx
            .update(|cx| TrustedWorktrees::try_get_global(cx))
            .context("missing trusted worktrees")?;
        trusted_worktrees.update(&mut cx, |trusted_worktrees, cx| {
            let worktree_store = this.read(cx).worktree_store().downgrade();
            let restricted_paths = envelope
                .payload
                .worktree_ids
                .into_iter()
                .map(WorktreeId::from_proto)
                .map(PathTrust::Worktree)
                .collect::<HashSet<_>>();
            trusted_worktrees.restrict(worktree_store, restricted_paths, cx);
        });
        Ok(proto::Ack {})
    }

    // Goes from host to client.
    async fn handle_find_search_candidates_chunk(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::FindSearchCandidatesChunk>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> {
        let buffer_store = this.read_with(&mut cx, |this, _| this.buffer_store.clone());
        BufferStore::handle_find_search_candidates_chunk(buffer_store, envelope, cx).await
    }

    // Goes from client to host.
    async fn handle_find_search_candidates_cancel(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::FindSearchCandidatesCancelled>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let buffer_store = this.read_with(&mut cx, |this, _| this.buffer_store.clone());
        BufferStore::handle_find_search_candidates_cancel(buffer_store, envelope, cx).await
    }

    async fn handle_create_buffer_for_peer(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::CreateBufferForPeer>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |this, cx| {
            this.buffer_store.update(cx, |buffer_store, cx| {
                buffer_store.handle_create_buffer_for_peer(
                    envelope,
                    this.replica_id(),
                    this.capability(),
                    cx,
                )
            })
        })
    }

    async fn handle_search_candidate_buffers(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::FindSearchCandidates>,
        mut cx: AsyncApp,
    ) -> Result<proto::Ack> {
        let peer_id = envelope.original_sender_id.unwrap_or(envelope.sender_id);
        let message = envelope.payload;
        let project_id = message.project_id;
        let path_style = this.read_with(&cx, |this, cx| this.path_style(cx));
        let query =
            SearchQuery::from_proto(message.query.context("missing query field")?, path_style)?;

        let handle = message.handle;
        let buffer_store = this.read_with(&cx, |this, _| this.buffer_store().clone());
        let client = this.read_with(&cx, |this, _| this.client());
        let task = cx.spawn(async move |cx| {
            let results = this.update(cx, |this, cx| {
                this.search_impl(query, cx).matching_buffers(cx)
            });
            let (batcher, batches) = crate::project_search::AdaptiveBatcher::new(cx.background_executor());
            let mut new_matches = Box::pin(results.rx);

            let sender_task = cx.background_executor().spawn({
                let client = client.clone();
                async move {
                    let mut batches = std::pin::pin!(batches);
                    while let Some(buffer_ids) = batches.next().await {
                        client
                            .request(proto::FindSearchCandidatesChunk {
                                handle,
                                peer_id: Some(peer_id),
                                project_id,
                                variant: Some(
                                    proto::find_search_candidates_chunk::Variant::Matches(
                                        proto::FindSearchCandidatesMatches { buffer_ids },
                                    ),
                                ),
                            })
                            .await?;
                    }
                    anyhow::Ok(())
                }
            });

            while let Some((buffer, _)) = new_matches.next().await {
                let is_private = buffer.read_with(cx, |buffer, _| {
                    buffer.file().is_some_and(|file| file.is_private())
                });
                if is_private {
                    continue;
                }

                let buffer_id = this.update(cx, |this, cx| {
                    this.create_buffer_for_peer(&buffer, peer_id, cx).to_proto()
                });
                batcher.push(buffer_id).await;
            }
            batcher.flush().await;

            sender_task.await?;

            let _ = client
                .request(proto::FindSearchCandidatesChunk {
                    handle,
                    peer_id: Some(peer_id),
                    project_id,
                    variant: Some(proto::find_search_candidates_chunk::Variant::Done(
                        proto::FindSearchCandidatesDone {},
                    )),
                })
                .await?;
            anyhow::Ok(())
        });
        buffer_store.update(&mut cx, |this, _| {
            this.register_ongoing_project_search((peer_id, handle), task);
        });

        Ok(proto::Ack {})
    }

    async fn handle_open_buffer_by_id(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::OpenBufferById>,
        mut cx: AsyncApp,
    ) -> Result<proto::OpenBufferResponse> {
        let peer_id = envelope.original_sender_id()?;
        let buffer_id = BufferId::new(envelope.payload.id)?;
        let buffer = this
            .update(&mut cx, |this, cx| this.open_buffer_by_id(buffer_id, cx))
            .await?;
        Project::respond_to_open_buffer_request(this, buffer, peer_id, &mut cx)
    }

    async fn handle_open_buffer_by_path(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::OpenBufferByPath>,
        mut cx: AsyncApp,
    ) -> Result<proto::OpenBufferResponse> {
        let peer_id = envelope.original_sender_id()?;
        let worktree_id = WorktreeId::from_proto(envelope.payload.worktree_id);
        let path = RelPath::from_unix_str(&envelope.payload.path)?.into();
        let open_buffer = this
            .update(&mut cx, |this, cx| {
                this.open_buffer(ProjectPath { worktree_id, path }, cx)
            })
            .await?;
        Project::respond_to_open_buffer_request(this, open_buffer, peer_id, &mut cx)
    }

    async fn handle_open_new_buffer(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::OpenNewBuffer>,
        mut cx: AsyncApp,
    ) -> Result<proto::OpenBufferResponse> {
        let buffer = this
            .update(&mut cx, |this, cx| this.create_buffer(None, true, cx))
            .await?;
        let peer_id = envelope.original_sender_id()?;

        Project::respond_to_open_buffer_request(this, buffer, peer_id, &mut cx)
    }

    fn respond_to_open_buffer_request(
        this: Entity<Self>,
        buffer: Entity<Buffer>,
        peer_id: proto::PeerId,
        cx: &mut AsyncApp,
    ) -> Result<proto::OpenBufferResponse> {
        this.update(cx, |this, cx| {
            let is_private = buffer
                .read(cx)
                .file()
                .map(|f| f.is_private())
                .unwrap_or_default();
            anyhow::ensure!(!is_private, ErrorCode::UnsharedItem);
            Ok(proto::OpenBufferResponse {
                buffer_id: this.create_buffer_for_peer(&buffer, peer_id, cx).into(),
            })
        })
    }

    fn create_buffer_for_peer(
        &mut self,
        buffer: &Entity<Buffer>,
        peer_id: proto::PeerId,
        cx: &mut App,
    ) -> BufferId {
        self.buffer_store
            .update(cx, |buffer_store, cx| {
                buffer_store.create_buffer_for_peer(buffer, peer_id, cx)
            })
            .detach_and_log_err(cx);
        buffer.read(cx).remote_id()
    }

    async fn handle_create_image_for_peer(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::CreateImageForPeer>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        this.update(&mut cx, |this, cx| {
            this.image_store.update(cx, |image_store, cx| {
                image_store.handle_create_image_for_peer(envelope, cx)
            })
        })
    }

    async fn handle_create_file_for_peer(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::CreateFileForPeer>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        use proto::create_file_for_peer::Variant;
        log::debug!("handle_create_file_for_peer: received message");

        let downloading_files: Arc<Mutex<HashMap<(WorktreeId, String), DownloadingFile>>> =
            this.update(&mut cx, |this, _| this.downloading_files.clone());

        match &envelope.payload.variant {
            Some(Variant::State(state)) => {
                log::debug!(
                    "handle_create_file_for_peer: got State: id={}, content_size={}",
                    state.id,
                    state.content_size
                );

                // Extract worktree_id and path from the File field
                if let Some(ref file) = state.file {
                    let worktree_id = WorktreeId::from_proto(file.worktree_id);
                    let path = file.path.clone();
                    let key = (worktree_id, path);
                    log::debug!("handle_create_file_for_peer: looking up key={:?}", key);

                    let empty_file_destination: Option<PathBuf> = {
                        let mut files = downloading_files.lock();
                        log::trace!(
                            "handle_create_file_for_peer: current downloading_files keys: {:?}",
                            files.keys().collect::<Vec<_>>()
                        );

                        if let Some(file_entry) = files.get_mut(&key) {
                            file_entry.total_size = state.content_size;
                            file_entry.file_id = Some(state.id);
                            log::debug!(
                                "handle_create_file_for_peer: updated file entry: total_size={}, file_id={}",
                                state.content_size,
                                state.id
                            );
                        } else {
                            log::warn!(
                                "handle_create_file_for_peer: key={:?} not found in downloading_files",
                                key
                            );
                        }

                        if state.content_size == 0 {
                            // No chunks will arrive for an empty file; write it now.
                            files.remove(&key).map(|entry| entry.destination_path)
                        } else {
                            None
                        }
                    };

                    if let Some(destination) = empty_file_destination {
                        log::debug!(
                            "handle_create_file_for_peer: writing empty file to {:?}",
                            destination
                        );
                        match smol::fs::write(&destination, &[] as &[u8]).await {
                            Ok(_) => log::info!(
                                "handle_create_file_for_peer: successfully wrote file to {:?}",
                                destination
                            ),
                            Err(e) => log::error!(
                                "handle_create_file_for_peer: failed to write empty file: {:?}",
                                e
                            ),
                        }
                    }
                } else {
                    log::warn!("handle_create_file_for_peer: State has no file field");
                }
            }
            Some(Variant::Chunk(chunk)) => {
                log::debug!(
                    "handle_create_file_for_peer: got Chunk: file_id={}, data_len={}",
                    chunk.file_id,
                    chunk.data.len()
                );

                // Extract data while holding the lock, then release it before await
                let (key_to_remove, write_info): (
                    Option<(WorktreeId, String)>,
                    Option<(PathBuf, Vec<u8>)>,
                ) = {
                    let mut files = downloading_files.lock();
                    let mut found_key: Option<(WorktreeId, String)> = None;
                    let mut write_data: Option<(PathBuf, Vec<u8>)> = None;

                    for (key, file_entry) in files.iter_mut() {
                        if file_entry.file_id == Some(chunk.file_id) {
                            file_entry.chunks.extend_from_slice(&chunk.data);
                            log::debug!(
                                "handle_create_file_for_peer: accumulated {} bytes, total_size={}",
                                file_entry.chunks.len(),
                                file_entry.total_size
                            );

                            if file_entry.chunks.len() as u64 >= file_entry.total_size
                                && file_entry.total_size > 0
                            {
                                let destination = file_entry.destination_path.clone();
                                let content = std::mem::take(&mut file_entry.chunks);
                                found_key = Some(key.clone());
                                write_data = Some((destination, content));
                            }
                            break;
                        }
                    }
                    (found_key, write_data)
                }; // MutexGuard is dropped here

                // Perform the async write outside the lock
                if let Some((destination, content)) = write_info {
                    log::debug!(
                        "handle_create_file_for_peer: writing {} bytes to {:?}",
                        content.len(),
                        destination
                    );
                    match smol::fs::write(&destination, &content).await {
                        Ok(_) => log::info!(
                            "handle_create_file_for_peer: successfully wrote file to {:?}",
                            destination
                        ),
                        Err(e) => log::error!(
                            "handle_create_file_for_peer: failed to write file: {:?}",
                            e
                        ),
                    }
                }

                // Remove the completed entry
                if let Some(key) = key_to_remove {
                    downloading_files.lock().remove(&key);
                    log::debug!("handle_create_file_for_peer: removed completed download entry");
                }
            }
            None => {
                log::warn!("handle_create_file_for_peer: got None variant");
            }
        }

        Ok(())
    }

}
