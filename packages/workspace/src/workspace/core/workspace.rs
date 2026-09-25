/// 顶层 Workspace entity。对齐 zed `Workspace` 但做了大幅简化。
/// Zed 的 Workspace ~3000 行（含 Pane、ItemHandle、ModalLayer、TeleportLayer 等），
/// AAgent 现阶段只持有 Dock + StatusBar + 中心区域。
pub struct Workspace {
    pub(super) weak_self: WeakEntity<Self>,
    /// 外部注册的 action callback 收集器。
    /// Zed 用 `Vec<Box<dyn Fn(Div, ...) -> Div>>` 在 render 顶层 div 上应用。
    /// 我们简化为 `Vec<Box<dyn ActionCallback>>`，render 时 chain on_action。
    pub(super) workspace_actions:
        Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>,
    pub(super) zoomed: Option<AnyWeakView>,
    /// 对齐 zed `previous_dock_drag_coordinates` — 坐标去重, 避免相同位置重复触发 resize。
    pub(super) previous_dock_drag_coordinates: Option<Point<Pixels>>,
    pub(super) zoomed_position: Option<DockPosition>,
    pub(super) maximized_pane: Option<WeakEntity<Pane>>,
    /// 中心 PaneGroup — 递归 split 树，装 Pane（每个 Pane 装 items）
    pub(super) center: PaneGroup,
    /// 三个 Dock 实例（左/底/右），每个装多个 Panel。
    pub(super) left_dock: Entity<Dock>,
    pub(super) bottom_dock: Entity<Dock>,
    pub(super) right_dock: Entity<Dock>,
    pub(super) panes: Vec<Entity<Pane>>,
    pub(super) panes_by_item: HashMap<EntityId, WeakEntity<Pane>>,
    pub(super) active_pane: Entity<Pane>,
    pub(super) last_active_center_pane: Option<WeakEntity<Pane>>,
    pub(super) last_active_view_id: Option<proto::ViewId>,
    /// 状态栏（含 PanelButtons + 普通状态项）
    pub(super) status_bar: Entity<StatusBar>,
    pub(crate) modal_layer: Entity<ModalLayer>, // 原样
    pub(super) toast_layer: Entity<ToastLayer>,
    /// 可选的窗口装饰（TitleBar）— 由外部 crate（title-bar）创建后注入。
    /// 对齐 zed `workspace.rs:1598 titlebar_item: Option<AnyView>`
    pub(super) titlebar_item: Option<AnyView>,
    pub(super) titlebar_focus_handle: FocusHandle,
    pub(super) region_focus_handles: RegionFocusHandles,
    pub(super) notifications: Notifications,
    pub(super) suppressed_notifications: HashSet<NotificationId>,
    pub(super) project: Entity<Project>,
    pub(super) follower_states: HashMap<CollaboratorId, FollowerState>,
    pub(crate) last_leaders_by_pane: HashMap<WeakEntity<Pane>, CollaboratorId>,
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
    /// Workspace 边界 — 用于 resize 计算右 dock / 底 dock 的尺寸。
    /// 通过 canvas element 更新（对齐 zed workspace.rs:L9669-L9706）
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
impl EventEmitter<Event> for Workspace {}
