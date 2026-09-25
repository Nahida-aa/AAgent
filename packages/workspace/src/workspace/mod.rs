// Workspace struct + 稳定状态的访问器
// workspace/
// ├── core.rs                    # Workspace 结构体定义、字段
// ├── read.rs                    # Workspace 自身字段的只读访问器（见下）
// │
// ├── item/
// │   ├── mod.rs
// │   ├── read.rs                # active_item / items / item_of_type / pane_for*
// │   ├── ops.rs                 # open_path / activate_item / close_item_*
// │   └── serialization.rs       # item 序列化（可选）
// │
// ├── pane/
// │   ├── mod.rs
// │   ├── read.rs                # panes / active_pane / focused_pane
// │   ├── ops.rs                 # split_pane / join_all_panes / remove_pane
// │   ├── navigation.rs          # activate_pane_in_direction / find_pane_in_direction
// │   └── events.rs              # handle_pane_event / set_active_pane
// │
// ├── dock/
// │   ├── mod.rs
// │   ├── read.rs                # left_dock / all_docks / dock_at_position / panel
// │   ├── sizing.rs              # dock_size / dock_flex_for_size / resize_*
// │   ├── toggle.rs              # toggle_dock / toggle_all_docks
// │   └── render.rs
// │
// ├── navigation/
// │   ├── mod.rs
// │   ├── read.rs                # recent_navigation_history / most_recent_active_path
// │   ├── ops.rs                 # go_back / go_forward / reopen_closed_item
// │   └── workspace_history.rs   # update_history（全局 HistoryManager）
// │
// ├── window/
// │   ├── title/
// │   ├── bounds.rs
// │   └── decorations.rs
// │
// ├── opening/
// ├── registries/
// ├── providers/
// ├── follow/
// ├── collab/
// ├── worktree/
// ├── actions/
// └── tests/
// 构造与恢复
mod construct;
// # worktree 列表 / root_paths
mod worktree;
// 序列化与恢复窗口几何
mod serialize;
// 「用户触发的事件处理」——名字就是这个意思
mod user_actions;
// impl Render for Workspace
mod render;
//
mod actions;
mod actions_impl;
mod active_call;
mod app_state;
mod close;
mod collab;

mod env;
mod event;
mod event_handling;
mod focus;
mod focus_regions;
mod followers;
// ---- 用户操作（保存/关闭/prompt/modal）----
mod helpers;
mod history;
mod lifecycle;
//  ---- 实体管理（item/pane/dock/panel）----
mod dock_management;
mod item_management;
mod pane_management;
mod panel_management;
// ---- 交互与渲染 ----
mod key_context;
mod modals;
mod multi_workspace_helpers;
mod navigation;
mod notification_ops;
mod open;
mod permalink;


mod terminal_provider;
mod toast;
mod window_title;
mod window_title_impl;
mod workspace_store;
// 只重导出真正需要对外暴露的
pub use core::{
    CloseIntent, Event, OpenMode, OpenVisible, Workspace, WorkspaceId,
};
pub use opening::{
    OpenOptions, OpenResult, WorkspaceMatching, open_paths, open_workspace_by_id,
};
pub use providers::{
    AnyActiveCall, DebuggerProvider, GlobalAnyActiveCall, TerminalProvider,
};
pub use registries::{register_project_item, register_serializable_item};
pub use window::title::{WindowTitleContext, WindowTitleNeeds};

use collections::HashMap;
use gpui::{AppContext, Context, WeakEntity};

pub use crate::collab::AutoWatch;
pub use crate::workspace_store::WorkspaceStore;
use crate::{Pane, workspace::followers::CollaboratorId};

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use collections::HashMap as _;
use gpui::{
    AnyView, App, Bounds, Context, Entity, EntityId, EventEmitter, FocusHandle, Global, Pixels,
    Point, Subscription, Task, WeakEntity, Window,
};

use crate::active_call::{ActiveCallEvent, AnyActiveCall, GlobalAnyActiveCall};
use crate::app_state::{ActiveWorktreeCreation, AppState, PreviousWorkspaceState};
use crate::collab::{FollowerState, ViewId};
use crate::dock::Dock;
use crate::item::{FollowableItemHandle, ItemHandle, WeakItemHandle};
use crate::modal_layer::ModalLayer;
use crate::multi_workspace::MultiWorkspace;
use crate::notifications::{NotificationId, Notifications};
use crate::pane::{Pane, SplitDirection};
use crate::pane_group::PaneGroup;
use crate::persistence::WorkspaceDb;
use crate::providers::{DebuggerProvider, TerminalProvider};
use crate::registries::{
    SerializableItemRegistry, register_project_item, register_serializable_item,
};
use crate::status_bar::StatusBar;
use crate::toast_layer::ToastLayer;
use crate::types::{
    CloseIntent, OpenMode, OpenOptions, OpenResult, OpenVisible, WorkspaceId, WorkspaceLocation,
    WorkspaceMatching,
};
use crate::window_title::{WindowTitleContext, WindowTitleNeeds};
use crate::workspace_store::CollaboratorId;

impl EventEmitter<crate::Event> for Workspace {}
impl EventEmitter<crate::workspace_error::WorkspaceError> for Workspace {}

pub(crate) struct DelayedDebouncedEditAction {
    pub(crate) task: Option<Task<()>>,
    pub(crate) cancel_channel: Option<futures::channel::oneshot::Sender<()>>,
}

impl DelayedDebouncedEditAction {
    fn new() -> DelayedDebouncedEditAction {
        DelayedDebouncedEditAction {
            task: None,
            cancel_channel: None,
        }
    }

    fn fire_new<F>(
        &mut self,
        delay: Duration,
        window: &mut Window,
        cx: &mut Context<Workspace>,
        func: F,
    ) where
        F: 'static
            + Send
            + FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) -> Task<Result<()>>,
    {
        if let Some(channel) = self.cancel_channel.take() {
            _ = channel.send(());
        }

        let (sender, mut receiver) = oneshot::channel::<()>();
        self.cancel_channel = Some(sender);

        let previous_task = self.task.take();
        self.task = Some(cx.spawn_in(window, async move |workspace, cx| {
            let mut timer = cx.background_executor().timer(delay).fuse();
            if let Some(previous_task) = previous_task {
                previous_task.await;
            }

            futures::select_biased! {
                _ = receiver => return,
                    _ = timer => {}
            }

            if let Some(result) = workspace
                .update_in(cx, |workspace, window, cx| (func)(workspace, window, cx))
                .log_err()
            {
                result.await.log_err();
            }
        }));
    }
}

/// Handles a workspace.
pub trait WorkspaceHandle {
    fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath>;
}

impl WorkspaceHandle for Entity<Workspace> {
    fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath> {
        self.read(cx)
            .worktrees(cx)
            .flat_map(|worktree| {
                let worktree_id = worktree.read(cx).id();
                worktree.read(cx).files(true, 0).map(move |f| ProjectPath {
                    worktree_id,
                    path: f.path.clone(),
                })
            })
            .collect::<Vec<_>>()
    }
}
pub const SERIALIZATION_THROTTLE_TIME: Duration = Duration::from_millis(200);
pub const MAX_RECENT_SELECTIONS: usize = 20;


impl Workspace {
    pub fn new(
        workspace_id: Option<WorkspaceId>,
        project: Entity<Project>,
        app_state: Arc<AppState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if let Some(trusted_worktrees) = TrustedWorktrees::try_get_global(cx) {
            cx.subscribe(&trusted_worktrees, |_, worktrees_store, e, cx| {
                if let TrustedWorktreesEvent::Trusted(..) = e {
                    // Do not persist auto trusted worktrees
                    if !ProjectSettings::get_global(cx).session.trust_all_worktrees {
                        worktrees_store.update(cx, |worktrees_store, cx| {
                            worktrees_store.schedule_serialization(
                                cx,
                                |new_trusted_worktrees, cx| {
                                    let timeout =
                                        cx.background_executor().timer(SERIALIZATION_THROTTLE_TIME);
                                    let db = WorkspaceDb::global(cx);
                                    cx.background_spawn(async move {
                                        timeout.await;
                                        db.save_trusted_worktrees(new_trusted_worktrees)
                                            .await
                                            .log_err();
                                    })
                                },
                            )
                        });
                    }
                }
            })
            .detach();

            cx.observe_global::<SettingsStore>(|_, cx| {
                if ProjectSettings::get_global(cx).session.trust_all_worktrees {
                    if let Some(trusted_worktrees) = TrustedWorktrees::try_get_global(cx) {
                        trusted_worktrees.update(cx, |trusted_worktrees, cx| {
                            trusted_worktrees.auto_trust_all(cx);
                        })
                    }
                }
            })
            .detach();
        }

        cx.subscribe_in(&project, window, move |this, _, event, window, cx| {
            match event {
                project::Event::RemoteIdChanged(_) => {
                    this.update_window_title(window, cx);
                }

                project::Event::CollaboratorLeft(peer_id) => {
                    this.collaborator_left(*peer_id, window, cx);
                }

                &project::Event::WorktreeRemoved(_) => {
                    this.update_window_title(window, cx);
                    this.serialize_workspace(window, cx);
                    this.update_history(cx);
                }

                &project::Event::WorktreeAdded(id) => {
                    this.update_window_title(window, cx);
                    if this
                        .project()
                        .read(cx)
                        .worktree_for_id(id, cx)
                        .is_some_and(|wt| wt.read(cx).is_visible())
                    {
                        this.serialize_workspace(window, cx);
                        this.update_history(cx);
                    }
                }
                project::Event::WorktreeUpdatedEntries(..) => {
                    this.update_window_title(window, cx);
                    this.serialize_workspace(window, cx);
                }

                project::Event::EntryRenamed {
                    old_abs_path,
                    new_abs_path,
                    ..
                } => {
                    if this.rename_persisted_navigation_history_paths(old_abs_path, new_abs_path) {
                        this.serialize_workspace(window, cx);
                    }
                }

                project::Event::DisconnectedFromHost => {
                    this.update_window_edited(window, cx);
                    let leaders_to_unfollow =
                        this.follower_states.keys().copied().collect::<Vec<_>>();
                    for leader_id in leaders_to_unfollow {
                        this.unfollow(leader_id, window, cx);
                    }
                }

                project::Event::DisconnectedFromRemote {
                    server_not_running: _,
                } => {
                    this.update_window_edited(window, cx);
                }

                project::Event::Closed => {
                    window.remove_window();
                }

                project::Event::DeletedEntry(_, entry_id) => {
                    for pane in this.panes.iter() {
                        pane.update(cx, |pane, cx| {
                            pane.handle_deleted_project_item(*entry_id, window, cx)
                        });
                    }
                }

                project::Event::Toast {
                    notification_id,
                    message,
                    link,
                } => this.show_notification(
                    NotificationId::named(notification_id.clone()),
                    cx,
                    |cx| {
                        let mut notification = MessageNotification::new(message.clone(), cx);
                        if let Some(link) = link {
                            notification = notification
                                .more_info_message(link.label)
                                .more_info_url(link.url);
                        }

                        cx.new(|_| notification)
                    },
                ),

                project::Event::HideToast { notification_id } => {
                    this.dismiss_notification(&NotificationId::named(notification_id.clone()), cx)
                }

                project::Event::LanguageServerPrompt(request) => {
                    struct LanguageServerPrompt;

                    this.show_notification(
                        NotificationId::composite::<LanguageServerPrompt>(request.id),
                        cx,
                        |cx| {
                            cx.new(|cx| {
                                notifications::LanguageServerPrompt::new(request.clone(), cx)
                            })
                        },
                    );
                }

                project::Event::AgentLocationChanged => {
                    this.handle_agent_location_changed(window, cx)
                }

                _ => {}
            }
            cx.notify()
        })
        .detach();

        cx.subscribe_in(
            &project.read(cx).breakpoint_store(),
            window,
            |workspace, _, event, window, cx| match event {
                BreakpointStoreEvent::BreakpointsUpdated(_, _)
                | BreakpointStoreEvent::BreakpointsCleared(_) => {
                    workspace.serialize_workspace(window, cx);
                }
                BreakpointStoreEvent::SetDebugLine | BreakpointStoreEvent::ClearDebugLines => {}
            },
        )
        .detach();
        if let Some(toolchain_store) = project.read(cx).toolchain_store() {
            cx.subscribe_in(
                &toolchain_store,
                window,
                |workspace, _, event, window, cx| match event {
                    ToolchainStoreEvent::CustomToolchainsModified => {
                        workspace.serialize_workspace(window, cx);
                    }
                    _ => {}
                },
            )
            .detach();
        }

        cx.on_focus_lost(window, |this, window, cx| {
            let focus_handle = window
                .focus_lost_restore_target(cx)
                .unwrap_or_else(|| this.fallback_focus_handle(window, cx));
            window.focus(&focus_handle, cx);
        })
        .detach();

        let weak_handle = cx.entity().downgrade();
        let pane_history_timestamp = Arc::new(AtomicUsize::new(0));

        let center_pane = cx.new(|cx| {
            let mut center_pane = Pane::new(
                weak_handle.clone(),
                project.clone(),
                pane_history_timestamp.clone(),
                None,
                NewFile.boxed_clone(),
                true,
                window,
                cx,
            );
            center_pane.set_can_split(Some(Arc::new(|_, _, _, _| true)));
            center_pane.set_should_display_welcome_page(true);
            center_pane
        });
        cx.subscribe_in(&center_pane, window, Self::handle_pane_event)
            .detach();

        window.focus(&center_pane.focus_handle(cx), cx);

        cx.emit(Event::PaneAdded(center_pane.clone()));

        let any_window_handle = window.window_handle();
        app_state.workspace_store.update(cx, |store, _| {
            store
                .workspaces
                .insert((any_window_handle, weak_handle.clone()));
        });

        let mut current_user = app_state.user_store.read(cx).watch_current_user();
        let mut connection_status = app_state.client.status();
        let _observe_current_user = cx.spawn_in(window, async move |this, cx| {
            current_user.next().await;
            connection_status.next().await;
            let mut stream =
                Stream::map(current_user, drop).merge(Stream::map(connection_status, drop));

            while stream.recv().await.is_some() {
                this.update(cx, |_, cx| cx.notify())?;
            }
            anyhow::Ok(())
        });

        // All leader updates are enqueued and then processed in a single task, so
        // that each asynchronous operation can be run in order.
        let (leader_updates_tx, mut leader_updates_rx) =
            mpsc::unbounded::<(PeerId, proto::UpdateFollowers)>();
        let _apply_leader_updates = cx.spawn_in(window, async move |this, cx| {
            while let Some((leader_id, update)) = leader_updates_rx.next().await {
                Self::process_leader_update(&this, leader_id, update, cx)
                    .await
                    .log_err();
            }

            Ok(())
        });

        cx.emit(Event::WorkspaceCreated(weak_handle.clone()));
        let modal_layer = cx.new(|_| ModalLayer::new());
        let toast_layer = cx.new(|_| ToastLayer::new());
        cx.subscribe(
            &modal_layer,
            |_, _, _: &modal_layer::ModalOpenedEvent, cx| {
                cx.emit(Event::ModalOpened);
            },
        )
        .detach();

        let left_dock = Dock::new(DockPosition::Left, modal_layer.clone(), window, cx);
        let bottom_dock = Dock::new(DockPosition::Bottom, modal_layer.clone(), window, cx);
        let right_dock = Dock::new(DockPosition::Right, modal_layer.clone(), window, cx);
        let left_dock_buttons = cx.new(|cx| PanelButtons::new(left_dock.clone(), cx));
        let bottom_dock_buttons = cx.new(|cx| PanelButtons::new(bottom_dock.clone(), cx));
        let right_dock_buttons = cx.new(|cx| PanelButtons::new(right_dock.clone(), cx));
        let multi_workspace = window
            .root::<MultiWorkspace>()
            .flatten()
            .map(|mw| mw.downgrade());
        let status_bar = cx.new(|cx| {
            let mut status_bar =
                StatusBar::new(&center_pane.clone(), multi_workspace.clone(), window, cx);
            status_bar.add_left_item(left_dock_buttons, window, cx);
            status_bar.add_right_item(right_dock_buttons, window, cx);
            status_bar.add_right_item(bottom_dock_buttons, window, cx);
            status_bar
        });

        let session_id = app_state.session.read(cx).id().to_owned();

        let mut active_call = None;
        if let Some(call) = GlobalAnyActiveCall::try_global(cx).cloned() {
            let subscriptions =
                vec![
                    call.0
                        .subscribe(window, cx, Box::new(Self::on_active_call_event)),
                ];
            active_call = Some((call, subscriptions));
        }

        let (serializable_items_tx, serializable_items_rx) =
            mpsc::unbounded::<Box<dyn SerializableItemHandle>>();
        let _items_serializer = cx.spawn_in(window, async move |this, cx| {
            Self::serialize_items(&this, serializable_items_rx, cx).await
        });

        let subscriptions = vec![
            cx.observe_window_activation(window, Self::on_window_activation_changed),
            cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                // Settings can only affect the title through these two values,
                // so skip the recomputation when they are unchanged.
                let settings = WorkspaceSettings::get_global(cx);
                let title_settings = (
                    settings.window_title_format.as_str(),
                    settings.window_title_separator.as_str(),
                );
                let last_title_settings = this
                    .last_window_title_settings
                    .as_ref()
                    .map(|(format, separator)| (format.as_str(), separator.as_str()));
                if last_title_settings != Some(title_settings) {
                    this.update_window_title(window, cx);
                }
            }),
            cx.subscribe_in(
                &project.read(cx).git_store().clone(),
                window,
                |this, _, event, window, cx| match event {
                    GitStoreEvent::ActiveRepositoryChanged(_)
                    | GitStoreEvent::RepositoryUpdated(
                        _,
                        RepositoryEvent::HeadChanged | RepositoryEvent::BranchListChanged,
                        true,
                    ) => {
                        if this.window_title_needs_branch(cx) {
                            this.update_window_title(window, cx);
                        }
                    }
                    _ => {}
                },
            ),
            cx.observe_window_bounds(window, move |this, window, cx| {
                if !window.is_window_active() {
                    return;
                }
                if this.bounds_save_task_queued.is_some() {
                    return;
                }
                this.bounds_save_task_queued = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    this.update_in(cx, |this, window, cx| {
                        this.save_window_bounds(window, cx).detach();
                        this.bounds_save_task_queued.take();
                    })
                    .ok();
                }));
                cx.notify();
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                let window_appearance = window.appearance();

                *SystemAppearance::global_mut(cx) = SystemAppearance(window_appearance.into());

                theme_settings::reload_theme(cx);
                theme_settings::reload_icon_theme(cx);
            }),
            cx.on_release({
                let weak_handle = weak_handle.clone();
                move |this, cx| {
                    this.app_state.workspace_store.update(cx, move |store, _| {
                        store.workspaces.retain(|(_, weak)| weak != &weak_handle);
                    })
                }
            }),
        ];

        cx.defer_in(window, move |this, window, cx| {
            this.update_window_title(window, cx);
            this.show_initial_notifications(cx);
        });

        let mut center = PaneGroup::new(center_pane.clone());
        center.set_is_center(true);
        center.mark_positions(cx);

        Workspace {
            weak_self: weak_handle.clone(),
            zoomed: None,
            zoomed_position: None,
            maximized_pane: None,
            previous_dock_drag_coordinates: None,
            center,
            panes: vec![center_pane.clone()],
            panes_by_item: Default::default(),
            active_pane: center_pane.clone(),
            last_active_center_pane: Some(center_pane.downgrade()),
            last_active_view_id: None,
            status_bar,
            modal_layer,
            toast_layer,
            titlebar_item: None,
            titlebar_focus_handle: cx.focus_handle(),
            region_focus_handles: RegionFocusHandles::new(cx),
            notifications: Notifications::default(),
            suppressed_notifications: HashSet::default(),
            left_dock,
            bottom_dock,
            right_dock,
            _panels_task: None,
            project: project.clone(),
            follower_states: Default::default(),
            last_leaders_by_pane: Default::default(),
            auto_watch: AutoWatch::Off,
            dispatching_keystrokes: Default::default(),
            window_edited: false,
            last_window_title: None,
            last_window_title_settings: None,
            dirty_items: Default::default(),
            active_call,
            database_id: workspace_id,
            app_state,
            _observe_current_user,
            _apply_leader_updates,
            _schedule_serialize_workspace: None,
            _serialize_workspace_task: None,
            _schedule_serialize_ssh_paths: None,
            leader_updates_tx,
            _subscriptions: subscriptions,
            pane_history_timestamp,
            workspace_actions: Default::default(),
            // This data will be incorrect, but it will be overwritten by the time it needs to be used.
            bounds: Default::default(),
            centered_layout: false,
            bounds_save_task_queued: None,
            on_prompt_for_new_path: None,
            on_prompt_for_open_path: None,
            terminal_provider: None,
            debugger_provider: None,
            serializable_items_tx,
            _items_serializer,
            session_id: Some(session_id),

            scheduled_tasks: Vec::new(),
            last_open_dock_positions: Vec::new(),
            removing: false,
            sidebar_focus_handle: None,
            multi_workspace,
            active_workspace_id: None,
            active_worktree_creation: ActiveWorktreeCreation::default(),
            open_in_dev_container: false,
            _dev_container_task: None,
            deferred_save_items: Vec::new(),
            persisted_recent_navigation_history: Vec::new(),
            last_active_project_path: None,
            restoring_workspace: false,
        }
    }

    pub fn new_local(
        abs_paths: Vec<PathBuf>,
        app_state: Arc<AppState>,
        requesting_window: Option<WindowHandle<MultiWorkspace>>,
        env: Option<HashMap<String, String>>,
        init: Option<Box<dyn FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + Send>>,
        open_mode: OpenMode,
        cx: &mut App,
    ) -> Task<anyhow::Result<OpenResult>> {
        let project_handle = Project::local(
            app_state.client.clone(),
            app_state.node_runtime.clone(),
            app_state.user_store.clone(),
            app_state.languages.clone(),
            app_state.fs.clone(),
            env,
            Default::default(),
            cx,
        );

        let db = WorkspaceDb::global(cx);
        let kvp = db::kvp::KeyValueStore::global(cx);
        cx.spawn(async move |cx| {
            let mut paths_to_open = Vec::with_capacity(abs_paths.len());
            for path in abs_paths.into_iter() {
                if let Some(canonical) = app_state.fs.canonicalize(&path).await.ok() {
                    paths_to_open.push(canonical)
                } else {
                    paths_to_open.push(path)
                }
            }

            let serialized_workspace = db.workspace_for_roots(paths_to_open.as_slice());

            if let Some(paths) = serialized_workspace.as_ref().map(|ws| &ws.paths) {
                paths_to_open = paths.ordered_paths().cloned().collect();
            }

            // Get project paths for all of the abs_paths
            let mut project_paths: Vec<(PathBuf, Option<ProjectPath>)> =
                Vec::with_capacity(paths_to_open.len());

            for path in paths_to_open.into_iter() {
                if let Some((_, project_entry)) = cx
                    .update(|cx| {
                        Workspace::project_path_for_path(project_handle.clone(), &path, true, cx)
                    })
                    .await
                    .log_err()
                {
                    project_paths.push((path, Some(project_entry)));
                } else {
                    project_paths.push((path, None));
                }
            }

            let workspace_id = if let Some(serialized_workspace) = serialized_workspace.as_ref() {
                serialized_workspace.id
            } else {
                db.next_id().await.unwrap_or_else(|_| Default::default())
            };

            let toolchains = db.toolchains(workspace_id).await?;

            for (toolchain, worktree_path, path) in toolchains {
                let toolchain_path = PathBuf::from(toolchain.path.clone().to_string());
                let Some(worktree_id) = project_handle.read_with(cx, |this, cx| {
                    this.find_worktree(&worktree_path, cx)
                        .and_then(|(worktree, rel_path)| {
                            if rel_path.is_empty() {
                                Some(worktree.read(cx).id())
                            } else {
                                None
                            }
                        })
                }) else {
                    // We did not find a worktree with a given path, but that's whatever.
                    continue;
                };
                if !app_state.fs.is_file(toolchain_path.as_path()).await {
                    continue;
                }

                project_handle
                    .update(cx, |this, cx| {
                        this.activate_toolchain(ProjectPath { worktree_id, path }, toolchain, cx)
                    })
                    .await;
            }
            if let Some(workspace) = serialized_workspace.as_ref() {
                project_handle.update(cx, |this, cx| {
                    for (scope, toolchains) in &workspace.user_toolchains {
                        for toolchain in toolchains {
                            this.add_toolchain(toolchain.clone(), scope.clone(), cx);
                        }
                    }
                });
            }

            let window_to_replace = match open_mode {
                OpenMode::NewWindow => None,
                _ => requesting_window,
            };

            let (window, workspace): (WindowHandle<MultiWorkspace>, Entity<Workspace>) =
                if let Some(window) = window_to_replace {
                    let centered_layout = serialized_workspace
                        .as_ref()
                        .map(|w| w.centered_layout)
                        .unwrap_or(false);

                    let workspace = window.update(cx, |multi_workspace, window, cx| {
                        let workspace = cx.new(|cx| {
                            let mut workspace = Workspace::new(
                                Some(workspace_id),
                                project_handle.clone(),
                                app_state.clone(),
                                window,
                                cx,
                            );

                            workspace.centered_layout = centered_layout;

                            // Call init callback to add items before window renders
                            if let Some(init) = init {
                                init(&mut workspace, window, cx);
                            }

                            workspace
                        });
                        match open_mode {
                            OpenMode::Activate => {
                                multi_workspace.activate(workspace.clone(), None, window, cx);
                            }
                            OpenMode::Add => {
                                multi_workspace.add(workspace.clone(), &*window, cx);
                            }
                            OpenMode::NewWindow => {
                                unreachable!()
                            }
                        }
                        workspace
                    })?;
                    (window, workspace)
                } else {
                    let window_bounds_override = window_bounds_env_override();

                    let (window_bounds, display) = if let Some(bounds) = window_bounds_override {
                        (Some(WindowBounds::Windowed(bounds)), None)
                    } else if let Some(workspace) = serialized_workspace.as_ref()
                        && let Some(display) = workspace.display
                        && let Some(bounds) = workspace.window_bounds.as_ref()
                    {
                        // Reopening an existing workspace - restore its saved bounds
                        (Some(bounds.0), Some(display))
                    } else if let Some((display, bounds)) =
                        persistence::read_default_window_bounds(&kvp)
                    {
                        // New or empty workspace - use the last known window bounds
                        (Some(bounds), Some(display))
                    } else {
                        // New window - let GPUI's default_bounds() handle cascading
                        (None, None)
                    };

                    // Use the serialized workspace to construct the new window
                    let mut options = cx.update(|cx| (app_state.build_window_options)(display, cx));
                    options.window_bounds = window_bounds;
                    let centered_layout = serialized_workspace
                        .as_ref()
                        .map(|w| w.centered_layout)
                        .unwrap_or(false);
                    let window = cx.open_window(options, {
                        let app_state = app_state.clone();
                        let project_handle = project_handle.clone();
                        move |window, cx| {
                            let workspace = cx.new(|cx| {
                                let mut workspace = Workspace::new(
                                    Some(workspace_id),
                                    project_handle,
                                    app_state,
                                    window,
                                    cx,
                                );
                                workspace.centered_layout = centered_layout;

                                // Call init callback to add items before window renders
                                if let Some(init) = init {
                                    init(&mut workspace, window, cx);
                                }

                                workspace
                            });
                            cx.new(|cx| MultiWorkspace::new(workspace, window, cx))
                        }
                    })?;
                    let workspace =
                        window.update(cx, |multi_workspace: &mut MultiWorkspace, _, _cx| {
                            multi_workspace.workspace().clone()
                        })?;
                    (window, workspace)
                };

            notify_if_database_failed(window, cx);
            // Check if this is an empty workspace (no paths to open)
            // An empty workspace is one where project_paths is empty
            let is_empty_workspace = project_paths.is_empty();
            // Check if serialized workspace has paths before it's moved
            let serialized_workspace_has_paths = serialized_workspace
                .as_ref()
                .map(|ws| !ws.paths.is_empty())
                .unwrap_or(false);

            let opened_items = window
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |_workspace: &mut Workspace, cx| {
                        open_items(serialized_workspace, project_paths, window, cx)
                    })
                })?
                .await
                .unwrap_or_default();

            // Restore default dock state for empty workspaces
            // Only restore if:
            // 1. This is an empty workspace (no paths), AND
            // 2. The serialized workspace either doesn't exist or has no paths
            if is_empty_workspace && !serialized_workspace_has_paths {
                if let Some(default_docks) = persistence::read_default_dock_state(&kvp) {
                    window
                        .update(cx, |_, window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                for (dock, serialized_dock) in [
                                    (&workspace.right_dock, &default_docks.right),
                                    (&workspace.left_dock, &default_docks.left),
                                    (&workspace.bottom_dock, &default_docks.bottom),
                                ] {
                                    dock.update(cx, |dock, cx| {
                                        dock.restore_serialized_state(
                                            serialized_dock.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                                cx.notify();
                            });
                        })
                        .log_err();
                }
            }

            window
                .update(cx, |_, _window, cx| {
                    workspace.update(cx, |this: &mut Workspace, cx| {
                        this.update_history(cx);
                    });
                })
                .log_err();

            if open_mode == OpenMode::NewWindow || open_mode == OpenMode::Activate {
                window
                    .update(cx, |_, window, _cx| {
                        window.activate_window();
                    })
                    .log_err();
            }

            // Auto-show the security modal if the project has restricted worktrees
            window
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |workspace, cx| {
                        workspace.show_worktree_trust_security_modal(false, window, cx);
                    });
                })
                .log_err();

            Ok(OpenResult {
                window,
                workspace,
                opened_items,
            })
        })
    }
    pub fn weak_handle(&self) -> WeakEntity<Self> { self.weak_self.clone() }
    pub fn project(&self) -> &Entity<Project> { &self.project }
    pub fn user_store(&self) -> &Entity<UserStore> { &self.app_state.user_store }
    pub fn app_state(&self) -> &Arc<AppState> { &self.app_state }
    pub fn path_style(&self, cx: &App) -> PathStyle { self.project.read(cx).path_style(cx) }

    pub fn client(&self) -> &Arc<Client> { &self.app_state.client }

    pub fn database_id(&self) -> Option<WorkspaceId> { self.database_id }

    pub fn session_id(&self) -> Option<String> { self.session_id.clone() }

    pub fn is_restoring(&self) -> bool { self.restoring_workspace }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_restoring_workspace(&mut self, restoring: bool) {
        self.restoring_workspace = restoring;
    }
    pub fn status_bar(&self) -> &Entity<StatusBar> { &self.status_bar }

    pub fn status_bar_visible(&self, cx: &App) -> bool { StatusBarSettings::get_global(cx).show }

    pub fn multi_workspace(&self) -> Option<&WeakEntity<MultiWorkspace>> {
        self.multi_workspace.as_ref()
    }

    pub fn set_multi_workspace(
        &mut self,
        multi_workspace: WeakEntity<MultiWorkspace>,
        active_workspace_id: Rc<Cell<EntityId>>,
        cx: &mut App,
    ) {
        self.status_bar.update(cx, |status_bar, cx| {
            status_bar.set_multi_workspace(multi_workspace.clone(), cx);
        });
        self.multi_workspace = Some(multi_workspace);
        self.active_workspace_id = Some(active_workspace_id);
    }

    pub fn set_panels_task(&mut self, task: Task<Result<()>>) { self._panels_task = Some(task); }

    pub fn take_panels_task(&mut self) -> Option<Task<Result<()>>> { self._panels_task.take() }

    pub fn active_worktree_creation(&self) -> &ActiveWorktreeCreation {
        &self.active_worktree_creation
    }

    pub fn set_active_worktree_creation(
        &mut self,
        label: Option<SharedString>,
        is_switch: bool,
        cx: &mut Context<Self>,
    ) {
        self.active_worktree_creation.label = label;
        self.active_worktree_creation.is_switch = is_switch;
        cx.emit(Event::WorktreeCreationChanged);
        cx.notify();
    }

    /// Captures the current workspace state for restoring after a worktree switch.
    /// This includes dock layout, open file paths, and the active file path.
    pub fn capture_state_for_worktree_switch(
        &self,
        window: &Window,
        fallback_focused_dock: Option<DockPosition>,
        cx: &App,
    ) -> PreviousWorkspaceState {
        let dock_structure = self.capture_dock_state(window, cx);
        let open_file_paths = self.open_item_abs_paths(cx);
        let active_file_path = self
            .active_item(cx)
            .and_then(|item| item.project_path(cx))
            .and_then(|pp| self.project().read(cx).absolute_path(&pp, cx));

        let focused_dock = self
            .focused_dock_position(window, cx)
            .or(fallback_focused_dock);

        PreviousWorkspaceState {
            dock_structure,
            open_file_paths,
            active_file_path,
            focused_dock,
        }
    }

    pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>> {
        window
            .root::<MultiWorkspace>()
            .flatten()
            .map(|multi_workspace| multi_workspace.read(cx).workspace().clone())
    }

    pub fn zoomed_item(&self) -> Option<&AnyWeakView> { self.zoomed.as_ref() }

    pub fn register_action<A: Action>(
        &mut self,
        callback: impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) + 'static,
    ) -> &mut Self {
        let callback = Arc::new(callback);

        self.workspace_actions.push(Box::new(move |div, _, _, cx| {
            let callback = callback.clone();
            div.on_action(cx.listener(move |workspace, event, window, cx| {
                (callback)(workspace, event, window, cx)
            }))
        }));
        self
    }
    pub fn register_action_renderer(
        &mut self,
        callback: impl Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div + 'static,
    ) -> &mut Self {
        self.workspace_actions.push(Box::new(callback));
        self
    }

    pub fn root_paths(&self, cx: &App) -> Vec<Arc<Path>> {
        let project = self.project().read(cx);
        project
            .visible_worktrees(cx)
            .map(|worktree| worktree.read(cx).abs_path())
            .collect::<Vec<_>>()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_random_database_id(&mut self) {
        self.database_id = Some(WorkspaceId(Uuid::new_v4().as_u64_pair().0 as i64));
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn test_new(project: Entity<Project>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use node_runtime::NodeRuntime;
        use session::Session;

        let client = project.read(cx).client();
        let user_store = project.read(cx).user_store();
        let workspace_store = cx.new(|cx| WorkspaceStore::new(client.clone(), cx));
        let session = cx.new(|cx| AppSession::new(Session::test(), cx));
        window.activate_window();
        let app_state = Arc::new(AppState {
            languages: project.read(cx).languages().clone(),
            workspace_store,
            client,
            user_store,
            fs: project.read(cx).fs().clone(),
            build_window_options: |_, _| Default::default(),
            node_runtime: NodeRuntime::unavailable(),
            session,
        });
        let workspace = Self::new(Default::default(), project, app_state, window, cx);
        workspace
            .active_pane
            .update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx));
        workspace
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_database_id(&mut self, id: WorkspaceId) { self.database_id = Some(id); }

    pub fn project_group_key(&self, cx: &App) -> ProjectGroupKey {
        self.project.read(cx).project_group_key(cx)
    }
}
/// Tracks worktree creation progress for the workspace.
/// Read by the title bar to show a loading indicator on the worktree button.
#[derive(Default)]
pub struct ActiveWorktreeCreation {
    pub label: Option<SharedString>,
    pub is_switch: bool,
}
/// Captured workspace state used when switching between worktrees.
/// Stores the layout and open files so they can be restored in the new workspace.
pub struct PreviousWorkspaceState {
    pub dock_structure: DockStructure,
    pub open_file_paths: Vec<PathBuf>,
    pub active_file_path: Option<PathBuf>,
    pub focused_dock: Option<DockPosition>,
}
