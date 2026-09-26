/// 顶层 Workspace entity。对齐 zed `Workspace` 但做了大幅简化。
/// Zed 的 Workspace ~3000 行（含 Pane、ItemHandle、ModalLayer、TeleportLayer 等），
/// AAgent 现阶段只持有 Dock + StatusBar + 中心区域。
use super::*;
pub struct Workspace {
    pub(crate) weak_self: WeakEntity<Self>,
    /// 外部注册的 action callback 收集器。
    /// Zed 用 `Vec<Box<dyn Fn(Div, ...) -> Div>>` 在 render 顶层 div 上应用。
    /// 我们简化为 `Vec<Box<dyn ActionCallback>>`，render 时 chain on_action。
    pub(crate) workspace_actions:
        Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>,
    pub(crate) zoomed: Option<AnyWeakView>,
    /// 对齐 zed `previous_dock_drag_coordinates` — 坐标去重, 避免相同位置重复触发 resize。
    pub(crate) previous_dock_drag_coordinates: Option<Point<Pixels>>,
    pub(crate) zoomed_position: Option<DockPosition>,
    pub(crate) maximized_pane: Option<WeakEntity<Pane>>,
    /// 中心 PaneGroup — 递归 split 树，装 Pane（每个 Pane 装 items）
    pub(crate) center: PaneGroup,
    /// 三个 Dock 实例（左/底/右），每个装多个 Panel。
    pub(crate) left_dock: Entity<Dock>,
    pub(crate) bottom_dock: Entity<Dock>,
    pub(crate) right_dock: Entity<Dock>,
    pub(crate) panes: Vec<Entity<Pane>>,
    pub(crate) panes_by_item: HashMap<EntityId, WeakEntity<Pane>>,
    pub(crate) active_pane: Entity<Pane>,
    pub(crate) last_active_center_pane: Option<WeakEntity<Pane>>,
    pub(crate) last_active_view_id: Option<proto::ViewId>,
    /// 状态栏（含 PanelButtons + 普通状态项）
    pub(crate) status_bar: Entity<StatusBar>,
    pub(crate) modal_layer: Entity<ModalLayer>, // 原样
    pub(crate) toast_layer: Entity<ToastLayer>,
    /// 可选的窗口装饰（TitleBar）— 由外部 crate（title-bar）创建后注入。
    /// 对齐 zed `workspace.rs:1598 titlebar_item: Option<AnyView>`
    pub(crate) titlebar_item: Option<AnyView>,
    pub(crate) titlebar_focus_handle: FocusHandle,
    pub(crate) region_focus_handles: RegionFocusHandles,
    pub(crate) notifications: Notifications,
    pub(crate) suppressed_notifications: HashSet<NotificationId>,
    pub(crate) project: Entity<Project>,
    pub(crate) follower_states: HashMap<CollaboratorId, FollowerState>,
    pub(crate) last_leaders_by_pane: HashMap<WeakEntity<Pane>, CollaboratorId>,
    pub(crate) auto_watch: AutoWatch,
    pub(crate) window_edited: bool,
    pub(crate) last_window_title: Option<String>,
    pub(crate) last_window_title_settings: Option<(String, String)>,
    pub(crate) dirty_items: HashMap<EntityId, Subscription>,
    pub(crate) active_call: Option<(GlobalAnyActiveCall, Vec<Subscription>)>,
    pub(crate) leader_updates_tx: mpsc::UnboundedSender<(PeerId, proto::UpdateFollowers)>,
    pub(crate) database_id: Option<WorkspaceId>,
    pub(crate) app_state: Arc<AppState>,
    pub(crate) dispatching_keystrokes: Rc<RefCell<DispatchingKeystrokes>>,
    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) _apply_leader_updates: Task<Result<()>>,
    pub(crate) _observe_current_user: Task<Result<()>>,
    pub(crate) _schedule_serialize_workspace: Option<Task<()>>,
    pub(crate) _serialize_workspace_task: Option<Task<()>>,
    pub(crate) _schedule_serialize_ssh_paths: Option<Task<()>>,
    pub(crate) pane_history_timestamp: Arc<AtomicUsize>,
    /// Workspace 边界 — 用于 resize 计算右 dock / 底 dock 的尺寸。
    /// 通过 canvas element 更新（对齐 zed workspace.rs:L9669-L9706）
    pub(crate) bounds: Bounds<Pixels>,
    pub centered_layout: bool, // 原样
    pub(crate) bounds_save_task_queued: Option<Task<()>>,
    pub(crate) on_prompt_for_new_path: Option<PromptForNewPath>,
    pub(crate) on_prompt_for_open_path: Option<PromptForOpenPath>,
    pub(crate) terminal_provider: Option<Box<dyn TerminalProvider>>,
    pub(crate) debugger_provider: Option<Arc<dyn DebuggerProvider>>,
    pub(crate) serializable_items_tx: UnboundedSender<Box<dyn SerializableItemHandle>>,
    pub(crate) _items_serializer: Task<Result<()>>,
    pub(crate) session_id: Option<String>,
    pub(crate) scheduled_tasks: Vec<Task<()>>,
    pub(crate) last_open_dock_positions: Vec<DockPosition>,
    pub(crate) removing: bool,
    pub(crate) open_in_dev_container: bool,
    pub(crate) _dev_container_task: Option<Task<Result<()>>>,
    pub(crate) _panels_task: Option<Task<Result<()>>>,
    pub(crate) sidebar_focus_handle: Option<FocusHandle>,
    pub(crate) multi_workspace: Option<WeakEntity<MultiWorkspace>>,
    pub(crate) active_workspace_id: Option<Rc<Cell<EntityId>>>,
    pub(crate) active_worktree_creation: ActiveWorktreeCreation,
    pub(crate) deferred_save_items: Vec<Box<dyn WeakItemHandle>>,
    pub(crate) persisted_recent_navigation_history: Vec<PathBuf>,
    pub(crate) last_active_project_path: Option<ProjectPath>,
    pub(crate) restoring_workspace: bool,
}
impl EventEmitter<Event> for Workspace {}

impl Focusable for Workspace {
    fn focus_handle(&self, cx: &App) -> FocusHandle { self.active_pane.focus_handle(cx) }
}

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
