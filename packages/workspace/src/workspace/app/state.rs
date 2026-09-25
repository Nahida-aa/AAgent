use crate::dock::DockPosition;
use crate::persistence::model::DockStructure;
use crate::workspace::workspace_store::WorkspaceStore;
use client::{Client, UserStore};
use gpui::{App, AsyncApp, Entity, Global, Window};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use project::{Fs, Project, WorktreeId};
use session::AppSession;
use std::sync::Arc;
use task::{DebugScenario, SharedTaskContext, SpawnInTerminal};
use uuid::Uuid;

pub struct AppState {
    pub languages: Arc<LanguageRegistry>,
    pub client: Arc<Client>,
    pub user_store: Entity<UserStore>,
    pub workspace_store: Entity<WorkspaceStore>,
    pub fs: Arc<dyn fs::Fs>,
    pub build_window_options: fn(Option<Uuid>, &mut App) -> WindowOptions,
    pub node_runtime: NodeRuntime,
    pub session: Entity<AppSession>,
}

pub(crate) struct GlobalAppState(Arc<AppState>);
impl Global for GlobalAppState {}

#[derive(Default)]
pub struct ActiveWorktreeCreation {
    pub label: Option<SharedString>,
    pub is_switch: bool,
}

pub struct PreviousWorkspaceState {
    pub dock_structure: DockStructure,
    pub open_file_paths: Vec<std::path::PathBuf>,
    pub active_file_path: Option<std::path::PathBuf>,
    pub focused_dock: Option<DockPosition>,
}

pub struct WorkspaceStore {
    workspaces: HashSet<(gpui::AnyWindowHandle, WeakEntity<Workspace>)>,
    client: Arc<Client>,
    _subscriptions: Vec<client::Subscription>,
}

impl AppState {
    #[track_caller]
    pub fn global(cx: &App) -> Arc<Self> { cx.global::<GlobalAppState>().0.clone() }
    pub fn try_global(cx: &App) -> Option<Arc<Self>> {
        cx.try_global::<GlobalAppState>()
            .map(|state| state.0.clone())
    }
    pub fn set_global(state: Arc<AppState>, cx: &mut App) { cx.set_global(GlobalAppState(state)); }

    #[cfg(any(test, feature = "test-support"))]
    pub fn test(cx: &mut App) -> Arc<Self> {
        use fs::Fs;
        use node_runtime::NodeRuntime;
        use session::Session;
        use settings::SettingsStore;

        if !cx.has_global::<SettingsStore>() {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
        }

        let fs = fs::FakeFs::new(cx.background_executor().clone());
        <dyn Fs>::set_global(fs.clone(), cx);
        let languages = Arc::new(LanguageRegistry::test(cx.background_executor().clone()));
        let clock = Arc::new(clock::FakeSystemClock::new());
        let http_client = http_client::FakeHttpClient::with_404_response();
        let client = Client::new(clock, http_client, cx);
        let session = cx.new(|cx| AppSession::new(Session::test(), cx));
        let user_store = cx.new(|cx| UserStore::new(client.clone(), cx));
        let workspace_store = cx.new(|cx| WorkspaceStore::new(client.clone(), cx));

        theme_settings::init(theme::LoadThemes::JustBase, cx);
        client::init(&client, cx);

        Arc::new(Self {
            client,
            fs,
            languages,
            user_store,
            workspace_store,
            node_runtime: NodeRuntime::unavailable(),
            build_window_options: |_, _| Default::default(),
            session,
        })
    }
}

impl WorkspaceStore {
    pub fn new(client: Arc<Client>, cx: &mut Context<Self>) -> Self {
        Self {
            workspaces: Default::default(),
            _subscriptions: vec![
                client.add_request_handler(cx.weak_entity(), Self::handle_follow),
                client.add_message_handler(cx.weak_entity(), Self::handle_update_followers),
            ],
            client,
        }
    }

    pub fn update_followers(
        &self,
        project_id: Option<u64>,
        update: proto::update_followers::Variant,
        cx: &App,
    ) -> Option<()> {
        let active_call = GlobalAnyActiveCall::try_global(cx)?;
        let room_id = active_call.0.room_id(cx)?;
        self.client
            .send(proto::UpdateFollowers {
                room_id,
                project_id,
                variant: Some(update),
            })
            .log_err()
    }

    pub async fn handle_follow(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::Follow>,
        mut cx: AsyncApp,
    ) -> Result<proto::FollowResponse> {
        this.update(&mut cx, |this, cx| {
            let follower = Follower {
                project_id: envelope.payload.project_id,
                peer_id: envelope.original_sender_id()?,
            };

            let mut response = proto::FollowResponse::default();

            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let handler_response =
                                workspace.handle_follow(follower.project_id, window, cx);
                            if let Some(active_view) = handler_response.active_view
                                && workspace.project.read(cx).remote_id() == follower.project_id
                            {
                                response.active_view = Some(active_view)
                            }
                        });
                    })
                    .is_ok()
            });

            Ok(response)
        })
    }

    async fn handle_update_followers(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateFollowers>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let leader_id = envelope.original_sender_id()?;
        let update = envelope.payload;

        this.update(&mut cx, |this, cx| {
            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let project_id = workspace.project.read(cx).remote_id();
                            if update.project_id != project_id && update.project_id.is_some() {
                                return;
                            }
                            workspace.handle_update_followers(
                                leader_id,
                                update.clone(),
                                window,
                                cx,
                            );
                        });
                    })
                    .is_ok()
            });
            Ok(())
        })
    }

    pub fn workspaces(&self) -> impl Iterator<Item = &WeakEntity<Workspace>> {
        self.workspaces.iter().map(|(_, weak)| weak)
    }

    pub fn workspaces_with_windows(
        &self,
    ) -> impl Iterator<Item = (gpui::AnyWindowHandle, &WeakEntity<Workspace>)> {
        self.workspaces.iter().map(|(window, weak)| (*window, weak))
    }
}
