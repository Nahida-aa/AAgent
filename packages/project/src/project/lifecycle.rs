use super::*;

use ::rpc::proto;
use anyhow::{Context as _, Result, anyhow};
use std::path::Path;
use util::ResultExt;

use gpui::{App, AppContext, AsyncApp, BorrowAppContext, Context};

use super::Project;
use super::state::ProjectClientState;

impl Project {
    pub(crate) fn release(&mut self, cx: &mut App) {
        if let Some(client) = self.remote_client.take() {
            let shutdown = client.update(cx, |client, cx| {
                client.shutdown_processes(
                    Some(proto::ShutdownRemoteServer {}),
                    cx.background_executor().clone(),
                )
            });

            cx.background_spawn(async move {
                if let Some(shutdown) = shutdown {
                    shutdown.await;
                }
            })
            .detach()
        }

        match &self.client_state {
            ProjectClientState::Local => {}
            ProjectClientState::Shared { .. } => {
                let _ = self.unshare_internal(cx);
            }
            ProjectClientState::Collab { remote_id, .. } => {
                let _ = self.collab_client.send(proto::LeaveProject {
                    project_id: *remote_id,
                });
                self.disconnected_from_host_internal(cx);
            }
        }
    }

    #[cfg(feature = "test-support")]
    pub fn shared(&mut self, project_id: u64, cx: &mut Context<Self>) -> Result<()> {
        anyhow::ensure!(
            matches!(self.client_state, ProjectClientState::Local),
            "project was already shared"
        );

        self.client_subscriptions.extend([
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&cx.entity(), &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.worktree_store, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.buffer_store, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.lsp_store, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.settings_observer, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.dap_store, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.breakpoint_store, &cx.to_async()),
            self.collab_client
                .subscribe_to_entity(project_id)?
                .set_entity(&self.git_store, &cx.to_async()),
        ]);

        self.buffer_store.update(cx, |buffer_store, cx| {
            buffer_store.shared(project_id, self.collab_client.clone().into(), cx)
        });
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.shared(project_id, self.collab_client.clone().into(), cx);
        });
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.shared(project_id, self.collab_client.clone().into(), cx)
        });
        self.breakpoint_store.update(cx, |breakpoint_store, _| {
            breakpoint_store.shared(project_id, self.collab_client.clone().into())
        });
        self.dap_store.update(cx, |dap_store, cx| {
            dap_store.shared(project_id, self.collab_client.clone().into(), cx);
        });
        self.task_store.update(cx, |task_store, cx| {
            task_store.shared(project_id, self.collab_client.clone().into(), cx);
        });
        self.settings_observer.update(cx, |settings_observer, cx| {
            settings_observer.shared(project_id, self.collab_client.clone().into(), cx)
        });
        self.git_store.update(cx, |git_store, cx| {
            git_store.shared(project_id, self.collab_client.clone().into(), cx)
        });

        self.client_state = ProjectClientState::Shared {
            remote_id: project_id,
        };

        cx.emit(Event::RemoteIdChanged(Some(project_id)));
        Ok(())
    }
    pub fn reshared(
        &mut self,
        message: proto::ResharedProject,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.buffer_store
            .update(cx, |buffer_store, _| buffer_store.forget_shared_buffers());
        self.set_collaborators_from_proto(message.collaborators, cx)?;

        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.send_project_updates(cx);
        });
        if let Some(remote_id) = self.remote_id() {
            self.git_store.update(cx, |git_store, cx| {
                git_store.shared(remote_id, self.collab_client.clone().into(), cx)
            });
        }
        cx.emit(Event::Reshared);
        Ok(())
    }
    pub fn rejoined(
        &mut self,
        message: proto::RejoinedProject,
        message_id: u32,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            for worktree_metadata in &message.worktrees {
                store
                    .clear_local_settings(WorktreeId::from_proto(worktree_metadata.id), cx)
                    .log_err();
            }
        });

        self.join_project_response_message_id = message_id;
        self.set_worktrees_from_proto(message.worktrees, cx)?;
        self.set_collaborators_from_proto(message.collaborators, cx)?;

        let project = cx.weak_entity();
        self.lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.set_language_server_statuses_from_proto(
                project,
                message.language_servers,
                message.language_server_capabilities,
                cx,
            )
        });
        self.enqueue_buffer_ordered_message(BufferOrderedMessage::Resync)
            .unwrap();
        cx.emit(Event::Rejoined);
        Ok(())
    }

    #[inline]
    pub fn unshare(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.unshare_internal(cx)?;
        cx.emit(Event::RemoteIdChanged(None));
        Ok(())
    }
    pub(crate) fn unshare_internal(&mut self, cx: &mut App) -> Result<()> {
        anyhow::ensure!(
            !self.is_via_collab(),
            "attempted to unshare a remote project"
        );

        if let ProjectClientState::Shared { remote_id, .. } = self.client_state {
            self.client_state = ProjectClientState::Local;
            self.collaborators.clear();
            self.client_subscriptions.clear();
            self.worktree_store.update(cx, |store, cx| {
                store.unshared(cx);
            });
            self.buffer_store.update(cx, |buffer_store, cx| {
                buffer_store.forget_shared_buffers();
                buffer_store.unshared(cx)
            });
            self.task_store.update(cx, |task_store, cx| {
                task_store.unshared(cx);
            });
            self.breakpoint_store.update(cx, |breakpoint_store, cx| {
                breakpoint_store.unshared(cx);
            });
            self.dap_store.update(cx, |dap_store, cx| {
                dap_store.unshared(cx);
            });
            self.settings_observer.update(cx, |settings_observer, cx| {
                settings_observer.unshared(cx);
            });
            self.git_store.update(cx, |git_store, cx| {
                git_store.unshared(cx);
            });

            self.collab_client
                .send(proto::UnshareProject {
                    project_id: remote_id,
                })
                .ok();
            Ok(())
        } else {
            anyhow::bail!("attempted to unshare an unshared project");
        }
    }
    pub fn disconnected_from_host(&mut self, cx: &mut Context<Self>) {
        if self.is_disconnected(cx) {
            return;
        }
        self.disconnected_from_host_internal(cx);
        cx.emit(Event::DisconnectedFromHost);
    }
    pub(crate) fn disconnected_from_host_internal(&mut self, cx: &mut App) {
        if let ProjectClientState::Collab {
            sharing_has_stopped,
            ..
        } = &mut self.client_state
        {
            *sharing_has_stopped = true;
            self.client_subscriptions.clear();
            self.collaborators.clear();
            self.worktree_store.update(cx, |store, cx| {
                store.disconnected_from_host(cx);
            });
            self.buffer_store.update(cx, |buffer_store, cx| {
                buffer_store.disconnected_from_host(cx)
            });
            self.lsp_store
                .update(cx, |lsp_store, _cx| lsp_store.disconnected_from_host());
        }
    }

    #[inline]
    pub fn set_role(&mut self, role: proto::ChannelRole, cx: &mut Context<Self>) {
        let new_capability =
            if role == proto::ChannelRole::Member || role == proto::ChannelRole::Admin {
                Capability::ReadWrite
            } else {
                Capability::ReadOnly
            };
        if let ProjectClientState::Collab { capability, .. } = &mut self.client_state {
            if *capability == new_capability {
                return;
            }

            *capability = new_capability;
            for buffer in self.opened_buffers(cx) {
                buffer.update(cx, |buffer, cx| buffer.set_capability(new_capability, cx));
            }
        }
    }
    pub fn close(&mut self, cx: &mut Context<Self>) { cx.emit(Event::Closed); }

    #[inline]
    pub fn is_disconnected(&self, cx: &App) -> bool {
        match &self.client_state {
            ProjectClientState::Collab {
                sharing_has_stopped,
                ..
            } => *sharing_has_stopped,
            ProjectClientState::Local if self.is_via_remote_server() => {
                self.remote_client_is_disconnected(cx)
            }
            _ => false,
        }
    }

    #[inline]
    fn remote_client_is_disconnected(&self, cx: &App) -> bool {
        self.remote_client
            .as_ref()
            .map(|remote| remote.read(cx).is_disconnected())
            .unwrap_or(false)
    }

    #[inline]
    pub fn capability(&self) -> Capability {
        match &self.client_state {
            ProjectClientState::Collab { capability, .. } => *capability,
            ProjectClientState::Shared { .. } | ProjectClientState::Local => Capability::ReadWrite,
        }
    }

    #[inline]
    pub fn is_read_only(&self, cx: &App) -> bool {
        self.is_disconnected(cx) || !self.capability().editable()
    }

    #[inline]
    #[inline]
    pub(crate) fn is_local(&self) -> bool {
        match &self.client_state {
            ProjectClientState::Local | ProjectClientState::Shared { .. } => {
                self.remote_client.is_none()
            }
            ProjectClientState::Collab { .. } => false,
        }
    }
    pub fn is_via_remote_server(&self) -> bool {
        match &self.client_state {
            ProjectClientState::Local | ProjectClientState::Shared { .. } => {
                self.remote_client.is_some()
            }
            ProjectClientState::Collab { .. } => false,
        }
    }

    /// Whether this project is from collab (not counting remote servers).
    #[inline]
    pub fn is_via_collab(&self) -> bool {
        match &self.client_state {
            ProjectClientState::Local | ProjectClientState::Shared { .. } => false,
            ProjectClientState::Collab { .. } => true,
        }
    }

    /// `!self.is_local()`
    #[inline]
    pub fn is_remote(&self) -> bool {
        debug_assert_eq!(
            !self.is_local(),
            self.is_via_collab() || self.is_via_remote_server()
        );
        !self.is_local()
    }

    #[inline]
    pub fn is_via_wsl_with_host_interop(&self, cx: &App) -> bool {
        match &self.client_state {
            ProjectClientState::Local | ProjectClientState::Shared { .. } => {
                matches!(
                    &self.remote_client, Some(remote_client)
                    if remote_client.read(cx).has_wsl_interop()
                )
            }
            _ => false,
        }
    }

    /// Whether this project is served by a WSL distribution.
    #[inline]
    pub fn is_via_wsl(&self, cx: &App) -> bool {
        matches!(
            self.remote_connection_options(cx),
            Some(RemoteConnectionOptions::Wsl(_))
        )
    }
    pub fn remote_id(&self) -> Option<u64> {
        match self.client_state {
            ProjectClientState::Local => None,
            ProjectClientState::Shared { remote_id, .. }
            | ProjectClientState::Collab { remote_id, .. } => Some(remote_id),
        }
    }

    #[inline]
    pub fn replica_id(&self) -> ReplicaId {
        match self.client_state {
            ProjectClientState::Collab { replica_id, .. } => replica_id,
            _ => {
                if self.remote_client.is_some() {
                    ReplicaId::REMOTE_SERVER
                } else {
                    ReplicaId::LOCAL
                }
            }
        }
    }

    #[inline]
    pub fn is_shared(&self) -> bool {
        match &self.client_state {
            ProjectClientState::Shared { .. } => true,
            ProjectClientState::Local => false,
            ProjectClientState::Collab { .. } => true,
        }
    }

    /// Returns the resolved version of `path`, that was found in `buffer`, if it exists.
    pub fn supports_terminal(&self, _cx: &App) -> bool {
        self.is_local() || self.is_via_remote_server()
    }

    #[inline]
    pub fn remote_connection_state(&self, cx: &App) -> Option<remote::ConnectionState> {
        self.remote_client
            .as_ref()
            .map(|remote| remote.read(cx).connection_state())
    }

    #[inline]
    pub fn remote_connection_options(&self, cx: &App) -> Option<RemoteConnectionOptions> {
        self.remote_client
            .as_ref()
            .map(|remote| remote.read(cx).connection_options())
    }

    /// Reveals the given path in the system file manager.
    ///
    /// On Windows with a WSL remote connection, this converts the POSIX path
    /// to a Windows UNC path before revealing.
    pub fn reveal_path(&self, path: &Path, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        if let Some(RemoteConnectionOptions::Wsl(wsl_options)) = self.remote_connection_options(cx)
        {
            let path = path.to_path_buf();
            cx.spawn(async move |_, cx| {
                wsl_path_to_windows_path(&wsl_options, &path)
                    .await
                    .map(|windows_path| cx.update(|cx| cx.reveal_path(&windows_path)))
            })
            .detach_and_log_err(cx);
            return;
        }

        cx.reveal_path(path);
    }

    #[inline]
    pub fn set_active_path(&mut self, entry: Option<ProjectPath>, cx: &mut Context<Self>) {
        let new_active_entry = entry.and_then(|project_path| {
            let worktree = self.worktree_for_id(project_path.worktree_id, cx)?;
            let entry = worktree.read(cx).entry_for_path(&project_path.path)?;
            Some(entry.id)
        });
        if new_active_entry != self.active_entry {
            self.active_entry = new_active_entry;
            self.lsp_store.update(cx, |lsp_store, _| {
                lsp_store.set_active_entry(new_active_entry);
            });
            cx.emit(Event::ActiveEntryChanged(new_active_entry));
        }
    }

    pub fn language_servers_running_disk_based_diagnostics<'a>(
        &'a self,
        cx: &'a App,
    ) -> impl Iterator<Item = LanguageServerId> + 'a {
        self.lsp_store
            .read(cx)
            .language_servers_running_disk_based_diagnostics()
    }
    pub fn disable_worktree_scanner(&mut self, cx: &mut Context<Self>) {
        self.worktree_store.update(cx, |worktree_store, _cx| {
            worktree_store.disable_scanner();
        });
    }
}
