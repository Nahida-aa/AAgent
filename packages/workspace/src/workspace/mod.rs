mod actions;
mod actions_impl;
mod app_state;
mod close;
mod collaboration;
mod docks;
mod events;
mod focus;
mod followers;
mod helpers;
mod history;
mod items;
mod key_context;
mod modals;
mod multi_workspace_helpers;
mod notification_ops;
mod open;
mod panels;
mod panes;
mod permalinks;
mod render;
mod serialize;
mod terminal_provider;
mod toasts;
mod window_title;
mod workspace_store;
mod worktree;

pub struct Workspace {
    pub(super) weak_self: WeakEntity<Self>,
    pub(super) workspace_actions:
        Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>,
    pub(super) zoomed: Option<AnyWeakView>,
    pub(super) previous_dock_drag_coordinates: Option<Point<Pixels>>,
    pub(super) zoomed_position: Option<DockPosition>,
    pub(super) maximized_pane: Option<WeakEntity<Pane>>,
    pub(super) center: PaneGroup,
    pub(super) left_dock: Entity<Dock>,
    pub(super) bottom_dock: Entity<Dock>,
    pub(super) right_dock: Entity<Dock>,
    pub(super) panes: Vec<Entity<Pane>>,
    pub(super) panes_by_item: HashMap<EntityId, WeakEntity<Pane>>,
    pub(super) active_pane: Entity<Pane>,
    pub(super) last_active_center_pane: Option<WeakEntity<Pane>>,
    pub(super) last_active_view_id: Option<proto::ViewId>,
    pub(super) status_bar: Entity<StatusBar>,
    pub(crate) modal_layer: Entity<ModalLayer>, // 原样
    pub(super) toast_layer: Entity<ToastLayer>,
    pub(super) titlebar_item: Option<AnyView>,
    pub(super) titlebar_focus_handle: FocusHandle,
    pub(super) region_focus_handles: RegionFocusHandles,
    pub(super) notifications: Notifications,
    pub(super) suppressed_notifications: HashSet<NotificationId>,
    pub(super) project: Entity<Project>,
    pub(super) follower_states: HashMap<CollaboratorId, FollowerState>,
    pub(super) last_leaders_by_pane: HashMap<WeakEntity<Pane>, CollaboratorId>,
    pub(super) auto_watch: AutoWatch,
    pub(super) window_edited: bool,
    pub(super) last_window_title: Option<String>,
    pub(super) last_window_title_settings: Option<(String, String)>,
    pub(super) dirty_items: HashMap<EntityId, Subscription>,
    pub(super) active_call: Option<(GlobalAnyActiveCall, Vec<Subscription>)>,
    pub(super) leader_updates_tx: mpsc::UnboundedSender<(PeerId, proto::UpdateFollowers)>,
    pub(super) database_id: Option<WorkspaceId>,
    pub(super) app_state: Arc<AppState>,
    pub(super) dispatching_keystrokes: Rc<RefCell<DispatchingKeystrokes>>,
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) _apply_leader_updates: Task<Result<()>>,
    pub(super) _observe_current_user: Task<Result<()>>,
    pub(super) _schedule_serialize_workspace: Option<Task<()>>,
    pub(super) _serialize_workspace_task: Option<Task<()>>,
    pub(super) _schedule_serialize_ssh_paths: Option<Task<()>>,
    pub(super) pane_history_timestamp: Arc<AtomicUsize>,
    pub(super) bounds: Bounds<Pixels>,
    pub centered_layout: bool, // 原样
    pub(super) bounds_save_task_queued: Option<Task<()>>,
    pub(super) on_prompt_for_new_path: Option<PromptForNewPath>,
    pub(super) on_prompt_for_open_path: Option<PromptForOpenPath>,
    pub(super) terminal_provider: Option<Box<dyn TerminalProvider>>,
    pub(super) debugger_provider: Option<Arc<dyn DebuggerProvider>>,
    pub(super) serializable_items_tx: UnboundedSender<Box<dyn SerializableItemHandle>>,
    pub(super) _items_serializer: Task<Result<()>>,
    pub(super) session_id: Option<String>,
    pub(super) scheduled_tasks: Vec<Task<()>>,
    pub(super) last_open_dock_positions: Vec<DockPosition>,
    pub(super) removing: bool,
    pub(super) open_in_dev_container: bool,
    pub(super) _dev_container_task: Option<Task<Result<()>>>,
    pub(super) _panels_task: Option<Task<Result<()>>>,
    pub(super) sidebar_focus_handle: Option<FocusHandle>,
    pub(super) multi_workspace: Option<WeakEntity<MultiWorkspace>>,
    pub(super) active_workspace_id: Option<Rc<Cell<EntityId>>>,
    pub(super) active_worktree_creation: ActiveWorktreeCreation,
    pub(super) deferred_save_items: Vec<Box<dyn WeakItemHandle>>,
    pub(super) persisted_recent_navigation_history: Vec<PathBuf>,
    pub(super) last_active_project_path: Option<ProjectPath>,
    pub(super) restoring_workspace: bool,
}
