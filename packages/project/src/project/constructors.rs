use super::*;

use ::rpc::proto::REMOTE_SERVER_PROJECT_ID;
use ::rpc::proto::ShutdownRemoteServer;
use client::ProjectId;
use collections::{BTreeSet, HashMap};
use std::sync::Arc;

use super::Project;
use super::state::LocalProjectFlags;
use super::state::ProjectClientState;
use crate::AgentServerStore;
use crate::DisableAiSettings;
use crate::ManifestTree;
use crate::constants::CURRENT_PROJECT_FEATURES;
use crate::directory::DirectoryLister;
use crate::lsp_store::LspStore;
use crate::prettier_store::PrettierStore;
use crate::trusted_worktrees;
use crate::types::*;
use crate::{Event, ProjectEnvironmentEvent, ToastLink};
use client::Client;
use gpui::{App, AppContext as _, Context, Entity, TaskExt as _};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use util::rel_path::RelPath;

impl Project {
    pub fn local(
        client: Arc<Client>,
        node: NodeRuntime,
        user_store: Entity<client::UserStore>,
        languages: Arc<LanguageRegistry>,
        fs: Arc<dyn Fs>,
        env: Option<HashMap<String, String>>,
        flags: LocalProjectFlags,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let (tx, rx) = mpsc::unbounded();
            cx.spawn(async move |this, cx| Self::send_buffer_ordered_messages(this, rx, cx).await)
                .detach();
            let snippets = SnippetProvider::new(fs.clone(), BTreeSet::from_iter([]), cx);
            let worktree_store =
                cx.new(|cx| WorktreeStore::local(false, fs.clone(), WorktreeIdCounter::get(cx)));
            if flags.init_worktree_trust {
                trusted_worktrees::track_worktree_trust(
                    worktree_store.clone(),
                    None,
                    None,
                    None,
                    cx,
                );
            }
            cx.subscribe(&worktree_store, Self::on_worktree_store_event)
                .detach();

            let weak_self = cx.weak_entity();
            let context_server_store = cx.new(|cx| {
                ContextServerStore::local(
                    worktree_store.clone(),
                    Some(weak_self.clone()),
                    false,
                    cx,
                )
            });

            let environment = cx.new(|cx| {
                ProjectEnvironment::new(env, worktree_store.downgrade(), None, false, cx)
            });
            let manifest_tree = ManifestTree::new(worktree_store.clone(), cx);
            let toolchain_store = cx.new(|cx| {
                ToolchainStore::local(
                    languages.clone(),
                    worktree_store.clone(),
                    environment.clone(),
                    manifest_tree.clone(),
                    cx,
                )
            });

            let buffer_store = cx.new(|cx| BufferStore::local(worktree_store.clone(), cx));
            cx.subscribe(&buffer_store, Self::on_buffer_store_event)
                .detach();

            let bookmark_store =
                cx.new(|cx| BookmarkStore::new(worktree_store.clone(), buffer_store.clone(), cx));

            let breakpoint_store =
                cx.new(|_| BreakpointStore::local(worktree_store.clone(), buffer_store.clone()));

            let dap_store = cx.new(|cx| {
                DapStore::new_local(
                    client.http_client(),
                    node.clone(),
                    fs.clone(),
                    environment.clone(),
                    toolchain_store.read(cx).as_language_toolchain_store(),
                    worktree_store.clone(),
                    breakpoint_store.clone(),
                    false,
                    cx,
                )
            });
            cx.subscribe(&dap_store, Self::on_dap_store_event).detach();

            let image_store = cx.new(|cx| ImageStore::local(worktree_store.clone(), cx));
            cx.subscribe(&image_store, Self::on_image_store_event)
                .detach();

            let prettier_store = cx.new(|cx| {
                PrettierStore::new(
                    node.clone(),
                    fs.clone(),
                    languages.clone(),
                    worktree_store.clone(),
                    cx,
                )
            });

            let git_store = cx.new(|cx| {
                GitStore::local(
                    &worktree_store,
                    buffer_store.clone(),
                    environment.clone(),
                    fs.clone(),
                    cx,
                )
            });
            git_store.update(cx, |git_store, _| git_store.set_project(weak_self.clone()));

            let task_store = cx.new(|cx| {
                TaskStore::local(
                    buffer_store.downgrade(),
                    worktree_store.clone(),
                    toolchain_store.read(cx).as_language_toolchain_store(),
                    environment.clone(),
                    git_store.clone(),
                    cx,
                )
            });

            let settings_observer = cx.new(|cx| {
                SettingsObserver::new_local(
                    fs.clone(),
                    worktree_store.clone(),
                    task_store.clone(),
                    flags.watch_global_configs,
                    cx,
                )
            });
            cx.subscribe(&settings_observer, Self::on_settings_observer_event)
                .detach();

            let lsp_store = cx.new(|cx| {
                LspStore::new_local(
                    buffer_store.clone(),
                    worktree_store.clone(),
                    prettier_store.clone(),
                    toolchain_store
                        .read(cx)
                        .as_local_store()
                        .expect("Toolchain store to be local")
                        .clone(),
                    environment.clone(),
                    manifest_tree,
                    languages.clone(),
                    client.http_client(),
                    fs.clone(),
                    cx,
                )
            });

            let agent_server_store = cx.new(|cx| {
                AgentServerStore::local(
                    node.clone(),
                    fs.clone(),
                    environment.clone(),
                    client.http_client(),
                    cx,
                )
            });

            cx.subscribe(&lsp_store, Self::on_lsp_store_event).detach();

            Self {
                buffer_ordered_messages_tx: tx,
                collaborators: Default::default(),
                worktree_store,
                buffer_store,
                image_store,
                lsp_store,
                context_server_store,
                join_project_response_message_id: 0,
                client_state: ProjectClientState::Local,
                git_store,
                client_subscriptions: Vec::new(),
                _subscriptions: vec![cx.on_release(Self::release)],
                active_entry: None,
                snippets,
                languages,
                collab_client: client,
                task_store,
                user_store,
                settings_observer,
                fs,
                remote_client: None,
                bookmark_store,
                breakpoint_store,
                dap_store,
                agent_server_store,

                buffers_needing_diff: Default::default(),
                git_diff_debouncer: DebouncedDelay::new(),
                terminals: Terminals {
                    local_handles: Vec::new(),
                },
                node: Some(node),
                search_history: Self::new_search_history(),
                environment,
                remotely_created_models: Default::default(),

                search_included_history: Self::new_search_history(),
                search_excluded_history: Self::new_search_history(),

                toolchain_store: Some(toolchain_store),

                agent_location: None,
                downloading_files: Default::default(),
                last_worktree_paths: WorktreePaths::default(),
            }
        })
    }

    pub fn remote(
        remote: Entity<remote::RemoteClient>,
        client: Arc<Client>,
        node: NodeRuntime,
        user_store: Entity<client::UserStore>,
        languages: Arc<LanguageRegistry>,
        fs: Arc<dyn Fs>,
        init_worktree_trust: bool,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let (tx, rx) = mpsc::unbounded();
            cx.spawn(async move |this, cx| Self::send_buffer_ordered_messages(this, rx, cx).await)
                .detach();
            let snippets = SnippetProvider::new(fs.clone(), BTreeSet::from_iter([]), cx);

            let (remote_proto, path_style, connection_options) =
                remote.read_with(cx, |remote, _| {
                    (
                        remote.proto_client(),
                        remote.path_style(),
                        remote.connection_options(),
                    )
                });
            let worktree_store = cx.new(|cx| {
                WorktreeStore::remote(
                    false,
                    remote_proto.clone(),
                    REMOTE_SERVER_PROJECT_ID,
                    path_style,
                    WorktreeIdCounter::get(cx),
                )
            });

            cx.subscribe(&worktree_store, Self::on_worktree_store_event)
                .detach();
            if init_worktree_trust {
                trusted_worktrees::track_worktree_trust(
                    worktree_store.clone(),
                    Some(RemoteHostLocation::from(connection_options)),
                    None,
                    Some((remote_proto.clone(), ProjectId(REMOTE_SERVER_PROJECT_ID))),
                    cx,
                );
            }

            let weak_self = cx.weak_entity();

            let buffer_store = cx.new(|cx| {
                BufferStore::remote(
                    worktree_store.clone(),
                    remote.read(cx).proto_client(),
                    REMOTE_SERVER_PROJECT_ID,
                    cx,
                )
            });
            let image_store = cx.new(|cx| {
                ImageStore::remote(
                    worktree_store.clone(),
                    remote.read(cx).proto_client(),
                    REMOTE_SERVER_PROJECT_ID,
                    cx,
                )
            });
            cx.subscribe(&buffer_store, Self::on_buffer_store_event)
                .detach();
            let toolchain_store = cx.new(|cx| {
                ToolchainStore::remote(
                    REMOTE_SERVER_PROJECT_ID,
                    worktree_store.clone(),
                    remote.read(cx).proto_client(),
                    cx,
                )
            });

            let context_server_store = cx.new(|cx| {
                ContextServerStore::remote(
                    REMOTE_SERVER_PROJECT_ID,
                    remote.clone(),
                    worktree_store.clone(),
                    Some(weak_self.clone()),
                    cx,
                )
            });

            let environment = cx.new(|cx| {
                ProjectEnvironment::new(
                    None,
                    worktree_store.downgrade(),
                    Some(remote.downgrade()),
                    false,
                    cx,
                )
            });

            let lsp_store = cx.new(|cx| {
                LspStore::new_remote(
                    buffer_store.clone(),
                    worktree_store.clone(),
                    languages.clone(),
                    remote_proto.clone(),
                    REMOTE_SERVER_PROJECT_ID,
                    cx,
                )
            });
            cx.subscribe(&lsp_store, Self::on_lsp_store_event).detach();

            let bookmark_store =
                cx.new(|cx| BookmarkStore::new(worktree_store.clone(), buffer_store.clone(), cx));

            let breakpoint_store = cx.new(|_| {
                BreakpointStore::remote(
                    REMOTE_SERVER_PROJECT_ID,
                    remote_proto.clone(),
                    buffer_store.clone(),
                    worktree_store.clone(),
                )
            });

            let dap_store = cx.new(|cx| {
                DapStore::new_remote(
                    REMOTE_SERVER_PROJECT_ID,
                    remote.clone(),
                    breakpoint_store.clone(),
                    worktree_store.clone(),
                    node.clone(),
                    client.http_client(),
                    fs.clone(),
                    cx,
                )
            });

            let git_store = cx.new(|cx| {
                GitStore::remote(
                    &worktree_store,
                    buffer_store.clone(),
                    remote_proto.clone(),
                    REMOTE_SERVER_PROJECT_ID,
                    cx,
                )
            });
            git_store.update(cx, |git_store, _| git_store.set_project(weak_self.clone()));

            let task_store = cx.new(|cx| {
                TaskStore::remote(
                    buffer_store.downgrade(),
                    worktree_store.clone(),
                    toolchain_store.read(cx).as_language_toolchain_store(),
                    remote.read(cx).proto_client(),
                    REMOTE_SERVER_PROJECT_ID,
                    git_store.clone(),
                    cx,
                )
            });

            let settings_observer = cx.new(|cx| {
                SettingsObserver::new_remote(
                    fs.clone(),
                    worktree_store.clone(),
                    task_store.clone(),
                    Some(remote_proto.clone()),
                    false,
                    cx,
                )
            });
            cx.subscribe(&settings_observer, Self::on_settings_observer_event)
                .detach();

            let agent_server_store = cx.new(|_| {
                AgentServerStore::remote(
                    REMOTE_SERVER_PROJECT_ID,
                    remote.clone(),
                    worktree_store.clone(),
                )
            });

            cx.subscribe(&remote, Self::on_remote_client_event).detach();

            let this = Self {
                buffer_ordered_messages_tx: tx,
                collaborators: Default::default(),
                worktree_store,
                buffer_store,
                image_store,
                lsp_store,
                context_server_store,
                bookmark_store,
                breakpoint_store,
                dap_store,
                join_project_response_message_id: 0,
                client_state: ProjectClientState::Local,
                git_store,
                agent_server_store,
                client_subscriptions: Vec::new(),
                _subscriptions: vec![
                    cx.on_release(Self::release),
                    cx.on_app_quit(|this, cx| {
                        let shutdown = this.remote_client.take().and_then(|client| {
                            client.update(cx, |client, cx| {
                                client.shutdown_processes(
                                    Some(ShutdownRemoteServer {}),
                                    cx.background_executor().clone(),
                                )
                            })
                        });

                        cx.background_executor().spawn(async move {
                            if let Some(shutdown) = shutdown {
                                shutdown.await;
                            }
                        })
                    }),
                ],
                active_entry: None,
                snippets,
                languages,
                collab_client: client,
                task_store,
                user_store,
                settings_observer,
                fs,
                remote_client: Some(remote.clone()),
                buffers_needing_diff: Default::default(),
                git_diff_debouncer: DebouncedDelay::new(),
                terminals: Terminals {
                    local_handles: Vec::new(),
                },
                node: Some(node),
                search_history: Self::new_search_history(),
                environment,
                remotely_created_models: Default::default(),

                search_included_history: Self::new_search_history(),
                search_excluded_history: Self::new_search_history(),

                toolchain_store: Some(toolchain_store),
                agent_location: None,
                downloading_files: Default::default(),
                last_worktree_paths: WorktreePaths::default(),
            };

            // remote server -> local machine handlers
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &cx.entity());
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.buffer_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.worktree_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.lsp_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.dap_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.breakpoint_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.settings_observer);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.git_store);
            remote_proto.subscribe_to_entity(REMOTE_SERVER_PROJECT_ID, &this.agent_server_store);

            remote_proto.add_entity_message_handler(Self::handle_create_buffer_for_peer);
            remote_proto.add_entity_message_handler(Self::handle_create_image_for_peer);
            remote_proto.add_entity_message_handler(Self::handle_create_file_for_peer);
            remote_proto.add_entity_message_handler(Self::handle_update_worktree);
            remote_proto.add_entity_message_handler(Self::handle_update_project);
            remote_proto.add_entity_message_handler(Self::handle_toast);
            remote_proto.add_entity_message_handler(Self::handle_telemetry_event);
            remote_proto.add_entity_request_handler(Self::handle_language_server_prompt_request);
            remote_proto
                .add_entity_request_handler(Self::handle_language_server_show_document_request);
            remote_proto.add_entity_message_handler(Self::handle_hide_toast);
            remote_proto.add_entity_request_handler(Self::handle_update_buffer_from_remote_server);
            remote_proto.add_entity_request_handler(Self::handle_trust_worktrees);
            remote_proto.add_entity_request_handler(Self::handle_restrict_worktrees);
            remote_proto.add_entity_request_handler(Self::handle_find_search_candidates_chunk);

            remote_proto.add_entity_message_handler(Self::handle_find_search_candidates_cancel);
            BufferStore::init(&remote_proto);
            WorktreeStore::init_remote(&remote_proto);
            LspStore::init(&remote_proto);
            SettingsObserver::init(&remote_proto);
            TaskStore::init(Some(&remote_proto));
            ToolchainStore::init(&remote_proto);
            DapStore::init(&remote_proto, cx);
            BreakpointStore::init(&remote_proto);
            GitStore::init(&remote_proto);
            AgentServerStore::init_remote(&remote_proto);

            this
        })
    }

    #[cfg(feature = "test-support")]
    pub async fn example(
        root_paths: impl IntoIterator<Item = &std::path::Path>,
        cx: &mut gpui::AsyncApp,
    ) -> Entity<Project> {
        use clock::FakeSystemClock;

        let fs = RealFs::new(None, cx.background_executor().clone());
        let languages = LanguageRegistry::test(cx.background_executor().clone());
        let clock = Arc::new(FakeSystemClock::new());
        let http_client = http_client::FakeHttpClient::with_404_response();
        let client = cx.update(|cx| client::Client::new(clock, http_client.clone(), cx));
        let user_store = cx.new(|cx| UserStore::new(client.clone(), cx));
        let project = cx.update(|cx| {
            Project::local(
                client,
                node_runtime::NodeRuntime::unavailable(),
                user_store,
                Arc::new(languages),
                fs,
                None,
                LocalProjectFlags {
                    init_worktree_trust: false,
                    ..Default::default()
                },
                cx,
            )
        });
        for path in root_paths {
            let (tree, _): (Entity<Worktree>, _) = project
                .update(cx, |project, cx| {
                    project.find_or_create_worktree(path, true, cx)
                })
                .await
                .unwrap();
            tree.read_with(cx, |tree, _| tree.as_local().unwrap().scan_complete())
                .await;
        }
        project
    }
}
