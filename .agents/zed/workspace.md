```rust
1: pub mod active_file_name
2: pub mod dock
3: pub mod history_manager
4: pub mod invalid_item_view
5: pub mod item
6: mod modal_layer
7: mod multi_workspace
9: mod multi_workspace_tests
10: pub mod notifications
11: pub mod pane
12: pub mod pane_group
13: pub mod path_list
16: pub mod path_link
17: mod persistence
18: pub mod searchable
19: pub mod security_modal
20: pub mod shared_screen
22: pub mod focus_follows_mouse
23: mod status_bar
24: pub mod tasks
25: mod theme_preview
26: mod toast_layer
27: mod toolbar
28: pub mod welcome
29: pub mod workspace_error
30: mod workspace_settings
176: pub const SERIALIZATION_THROTTLE_TIME: Duration
177: pub const MAX_RECENT_SELECTIONS: usize
181: struct WindowTitleNeeds
182: file_path: bool
183: relative_path: bool
184: file_stem: bool
185: remote: bool
186: app_name: bool
187: branch: bool
190: impl WindowTitleNeeds
191: fn from_template(template: &str) -> Self
204: struct WindowTitleContext
205: project_name: String
206: file_name: Option<String>
207: file_path: Option<String>
208: relative_path: Option<String>
209: file_stem: Option<String>
210: remote_name: Option<String>
211: remote_host: Option<String>
212: app_name: &'static str
213: branch: Option<String>
216: enum WindowTitleTemplatePart<'a>
217: Literal
218: Variable
219: Separator
222: impl WindowTitleContext
223: fn value_for(&self, variable: &str) -> Option<&str>
241: fn parse_window_title_format(template: &str) -> Vec<WindowTitleTemplatePart<'_>>
281: fn render_window_title_format(
313: static ZED_WINDOW_SIZE: LazyLock<Option<Size<Pixels>>>
320: static ZED_WINDOW_POSITION: LazyLock<Option<Point<Pixels>>>
327: pub trait TerminalProvider
328: fn spawn(
336: pub trait DebuggerProvider
338: fn start_session(
348: fn spawn_task_or_modal(
356: fn task_scheduled(&self, cx: &mut App)
357: fn debug_scenario_scheduled(&self, cx: &mut App)
358: fn debug_scenario_scheduled_last(&self, cx: &App) -> bool
360: fn active_thread_state(&self, cx: &App) -> Option<ThreadStatus>
366: pub struct Open
371: pub create_new_window: Option<bool>
374: impl Open
380: impl Default for Open
381: fn default() -> Self
507: pub struct ActivatePane(pub usize)
513: pub struct MoveItemToPane
515: pub destination: usize
517: pub focus: bool
519: pub clone: bool
522: fn default_1() -> usize
530: pub struct MoveItemToPaneInDirection
532: pub direction: SplitDirection
534: pub focus: bool
536: pub clone: bool
543: pub struct NewFileSplit(pub SplitDirection)
545: fn default_right() -> SplitDirection
553: pub struct SaveAll
555: pub save_intent: Option<SaveIntent>
562: pub struct Save
564: pub save_intent: Option<SaveIntent>
570: pub struct FocusCenterPane
576: pub struct CloseAllItemsAndPanes
578: pub save_intent: Option<SaveIntent>
585: pub struct CloseInactiveTabsAndPanes
587: pub save_intent: Option<SaveIntent>
594: pub struct CloseItemInAllPanes
596: pub save_intent: Option<SaveIntent>
598: pub close_pinned: bool
604: pub struct SendKeystrokes(pub String)
619: pub struct ToggleFileFinder
621: pub separate_history: bool
623: pub include_ignored: Option<bool>
630: pub struct NewCenterTerminal
633: pub local: bool
640: pub struct NewTerminal
643: pub local: bool
650: pub struct IncreaseActiveDockSize
653: pub px: u32
660: pub struct DecreaseActiveDockSize
663: pub px: u32
670: pub struct IncreaseOpenDocksSize
673: pub px: u32
680: pub struct DecreaseOpenDocksSize
683: pub px: u32
719: pub enum CloseIntent
721: Quit
723: CloseWindow
725: ReplaceWindow
729: pub struct Toast
730: id: NotificationId
731: message: Cow<'static, str>
732: autohide: bool
733: on_click: Option<(Cow<'static, str>, Arc<dyn Fn(&mut Window, &mut App)>)>
736: impl Toast
737: pub fn new<I: Into<Cow<'static, str>>>(id: NotificationId, message: I) -> Self
746: pub fn on_click<F, M>(mut self, message: M, on_click: F) -> Self
755: pub fn autohide(mut self) -> Self
762: pub fn open_file_permalink(
773: pub fn copy_file_permalink(
783: fn handle_file_permalink(
830: impl PartialEq for Toast
831: fn eq(&self, other: &Self) -> bool
842: pub struct OpenTerminal
843: pub working_directory: PathBuf
846: pub local: bool
862: pub struct WorkspaceId(i64)
864: impl WorkspaceId
865: pub fn from_i64(value: i64) -> Self
870: impl StaticColumnCount for WorkspaceId
871: impl Bind for WorkspaceId
872: fn bind(&self, statement: &Statement, start_index: i32) -> Result<i32>
876: impl Column for WorkspaceId
877: fn column(statement: &mut Statement, start_index: i32) -> Result<(Self, i32)>
883: impl From<WorkspaceId> for i64
884: fn from(val: WorkspaceId) -> Self
889: fn prompt_and_open_paths(
947: pub fn prompt_for_open_path_and_open(
991: pub fn init(app_state: Arc<AppState>, cx: &mut App)
1049: struct ProjectItemRegistry
1050: build_project_item_fns_by_type: TypeIdHashMap<BuildProjectItemFn>
1051: build_project_item_for_path_fns: Vec<BuildProjectItemForPathFn>
1054: impl ProjectItemRegistry
1055: fn register<T: ProjectItem>(&mut self)
1132: fn open_path(
1150: fn build_item<T: project::ProjectItem>(
1168: impl Global for ProjectItemRegistry
1173: pub fn register_project_item<I: ProjectItem>(cx: &mut App)
1178: pub struct FollowableViewRegistry(TypeIdHashMap<FollowableViewDescriptor>)
1180: struct FollowableViewDescriptor
1181: from_state_proto: fn(
1188: to_followable_view: fn(&AnyView) -> Box<dyn FollowableItemHandle>
1191: impl Global for FollowableViewRegistry
1193: impl FollowableViewRegistry
1194: pub fn register<I: FollowableItem>(cx: &mut App)
1209: pub fn from_state_proto(
1223: pub fn to_followable_view(
1235: struct SerializableItemDescriptor
1236: deserialize: fn(
1244: cleanup: fn(WorkspaceId, Vec<ItemId>, &mut Window, &mut App) -> Task<Result<()>>
1245: view_to_serializable_item: fn(AnyView) -> Box<dyn SerializableItemHandle>
1249: struct SerializableItemRegistry
1250: descriptors_by_kind: HashMap<Arc<str>, SerializableItemDescriptor>
1251: descriptors_by_type: TypeIdHashMap<SerializableItemDescriptor>
1254: impl Global for SerializableItemRegistry
1256: impl SerializableItemRegistry
1257: fn deserialize(
1276: fn cleanup(
1293: fn view_to_serializable_item_handle(
1302: fn descriptor(item_kind: &str, cx: &App) -> Option<SerializableItemDescriptor>
1308: pub fn register_serializable_item<I: SerializableItem>(cx: &mut App)
1331: pub struct AppState
1332: pub languages: Arc<LanguageRegistry>
1333: pub client: Arc<Client>
1334: pub user_store: Entity<UserStore>
1335: pub workspace_store: Entity<WorkspaceStore>
1336: pub fs: Arc<dyn fs::Fs>
1337: pub build_window_options: fn(Option<Uuid>, &mut App) -> WindowOptions
1338: pub node_runtime: NodeRuntime
1339: pub session: Entity<AppSession>
1342: struct GlobalAppState(Arc<AppState>)
1344: impl Global for GlobalAppState
1349: pub struct ActiveWorktreeCreation
1350: pub label: Option<SharedString>
1351: pub is_switch: bool
1356: pub struct PreviousWorkspaceState
1357: pub dock_structure: DockStructure
1358: pub open_file_paths: Vec<PathBuf>
1359: pub active_file_path: Option<PathBuf>
1360: pub focused_dock: Option<DockPosition>
1363: pub struct WorkspaceStore
1364: workspaces: HashSet<(gpui::AnyWindowHandle, WeakEntity<Workspace>)>
1365: client: Arc<Client>
1366: _subscriptions: Vec<client::Subscription>
1370: pub enum CollaboratorId
1371: PeerId
1372: Agent
1375: impl From<PeerId> for CollaboratorId
1376: fn from(peer_id: PeerId) -> Self
1381: impl From<&PeerId> for CollaboratorId
1382: fn from(peer_id: &PeerId) -> Self
1388: struct Follower
1389: project_id: Option<u64>
1390: peer_id: PeerId
1393: impl AppState
1395: pub fn global(cx: &App) -> Arc<Self>
1398: pub fn try_global(cx: &App) -> Option<Arc<Self>>
1402: pub fn set_global(state: Arc<AppState>, cx: &mut App)
1407: pub fn test(cx: &mut App) -> Arc<Self>
1444: struct DelayedDebouncedEditAction
1445: task: Option<Task<()>>
1446: cancel_channel: Option<oneshot::Sender<()>>
1449: impl DelayedDebouncedEditAction
1450: fn new() -> DelayedDebouncedEditAction
1457: fn fire_new<F>(
1497: pub enum Event
1498: PaneAdded
1499: PaneRemoved
1500: ItemAdded
1503: ActiveItemChanged
1504: ItemRemoved
1507: UserSavedItem
1512: ContactRequestedJoin
1513: WorkspaceCreated
1514: OpenBundledFile
1519: ZoomChanged
1520: ModalOpened
1521: Activate
1522: PanelAdded
1523: WorktreeCreationChanged
1529: pub enum OpenVisible
1531: All
1533: None
1535: OnlyFiles
1537: OnlyDirectories
1540: enum WorkspaceLocation
1542: Location
1544: None
1567: struct DispatchingKeystrokes
1568: dispatched: HashSet<Vec<Keystroke>>
1569: queue: VecDeque<Keystroke>
1570: task: Option<Shared<Task<()>>>
1579: pub struct Workspace
1580: weak_self: WeakEntity<Self>
1581: workspace_actions: Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>
1582: zoomed: Option<AnyWeakView>
1583: previous_dock_drag_coordinates: Option<Point<Pixels>>
1584: zoomed_position: Option<DockPosition>
1585: maximized_pane: Option<WeakEntity<Pane>>
1586: center: PaneGroup
1587: left_dock: Entity<Dock>
1588: bottom_dock: Entity<Dock>
1589: right_dock: Entity<Dock>
1590: panes: Vec<Entity<Pane>>
1591: panes_by_item: HashMap<EntityId, WeakEntity<Pane>>
1592: active_pane: Entity<Pane>
1593: last_active_center_pane: Option<WeakEntity<Pane>>
1594: last_active_view_id: Option<proto::ViewId>
1595: status_bar: Entity<StatusBar>
1596: pub(crate) modal_layer: Entity<ModalLayer>
1597: toast_layer: Entity<ToastLayer>
1598: titlebar_item: Option<AnyView>
1599: titlebar_focus_handle: FocusHandle
1600: region_focus_handles: RegionFocusHandles
1601: notifications: Notifications
1602: suppressed_notifications: HashSet<NotificationId>
1603: project: Entity<Project>
1604: follower_states: HashMap<CollaboratorId, FollowerState>
1605: last_leaders_by_pane: HashMap<WeakEntity<Pane>, CollaboratorId>
1606: auto_watch: AutoWatch
1607: window_edited: bool
1608: last_window_title: Option<String>
1612: last_window_title_settings: Option<(String, String)>
1613: dirty_items: HashMap<EntityId, Subscription>
1614: active_call: Option<(GlobalAnyActiveCall, Vec<Subscription>)>
1615: leader_updates_tx: mpsc::UnboundedSender<(PeerId, proto::UpdateFollowers)>
1616: database_id: Option<WorkspaceId>
1617: app_state: Arc<AppState>
1618: dispatching_keystrokes: Rc<RefCell<DispatchingKeystrokes>>
1619: _subscriptions: Vec<Subscription>
1620: _apply_leader_updates: Task<Result<()>>
1621: _observe_current_user: Task<Result<()>>
1622: _schedule_serialize_workspace: Option<Task<()>>
1623: _serialize_workspace_task: Option<Task<()>>
1624: _schedule_serialize_ssh_paths: Option<Task<()>>
1625: pane_history_timestamp: Arc<AtomicUsize>
1626: bounds: Bounds<Pixels>
1627: pub centered_layout: bool
1628: bounds_save_task_queued: Option<Task<()>>
1629: on_prompt_for_new_path: Option<PromptForNewPath>
1630: on_prompt_for_open_path: Option<PromptForOpenPath>
1631: terminal_provider: Option<Box<dyn TerminalProvider>>
1632: debugger_provider: Option<Arc<dyn DebuggerProvider>>
1633: serializable_items_tx: UnboundedSender<Box<dyn SerializableItemHandle>>
1634: _items_serializer: Task<Result<()>>
1635: session_id: Option<String>
1636: scheduled_tasks: Vec<Task<()>>
1637: last_open_dock_positions: Vec<DockPosition>
1638: removing: bool
1639: open_in_dev_container: bool
1640: _dev_container_task: Option<Task<Result<()>>>
1641: _panels_task: Option<Task<Result<()>>>
1642: sidebar_focus_handle: Option<FocusHandle>
1643: multi_workspace: Option<WeakEntity<MultiWorkspace>>
1650: active_workspace_id: Option<Rc<Cell<EntityId>>>
1651: active_worktree_creation: ActiveWorktreeCreation
1652: deferred_save_items: Vec<Box<dyn WeakItemHandle>>
1653: persisted_recent_navigation_history: Vec<PathBuf>
1654: last_active_project_path: Option<ProjectPath>
1655: restoring_workspace: bool
1658: impl EventEmitter<Event> for Workspace
1661: pub struct ViewId
1662: pub creator: CollaboratorId
1663: pub id: u64
1666: pub struct FollowerState
1667: center_pane: Entity<Pane>
1668: dock_pane: Option<Entity<Pane>>
1669: active_view_id: Option<ViewId>
1670: items_by_leader_view_id: HashMap<ViewId, FollowerView>
1674: pub enum AutoWatch
1675: Off
1676: Active
1677: Paused
1680: impl AutoWatch
1681: pub fn enabled(&self) -> bool
1686: struct FollowerView
1687: view: Box<dyn FollowableItemHandle>
1688: location: Option<proto::PanelId>
1692: pub enum OpenMode
1694: NewWindow
1696: Add
1699: Activate
1702: impl Workspace
1703: pub fn new(
2161: pub fn new_local(
2445: pub fn project_group_key(&self, cx: &App) -> ProjectGroupKey
2449: pub fn weak_handle(&self) -> WeakEntity<Self>
2453: pub fn left_dock(&self) -> &Entity<Dock>
2457: pub fn bottom_dock(&self) -> &Entity<Dock>
2461: pub fn set_bottom_dock_layout(
2476: pub fn right_dock(&self) -> &Entity<Dock>
2480: pub fn all_docks(&self) -> [&Entity<Dock>; 3]
2484: pub fn capture_dock_state(&self, _window: &Window, cx: &App) -> DockStructure
2527: pub fn set_dock_structure(
2544: pub fn finish_dock_restoration(&self, cx: &mut App)
2554: pub fn focused_dock_position(&self, window: &Window, cx: &App) -> Option<DockPosition>
2567: pub fn active_worktree_creation(&self) -> &ActiveWorktreeCreation
2571: pub fn set_active_worktree_creation(
2585: pub fn capture_state_for_worktree_switch(
2610: pub fn open_item_abs_paths(&self, cx: &App) -> Vec<PathBuf>
2619: pub fn dock_at_position(&self, position: DockPosition) -> &Entity<Dock>
2627: pub fn agent_panel_position(&self, cx: &App) -> Option<DockPosition>
2634: pub fn panel_size_state<T: Panel>(&self, cx: &App) -> Option<dock::PanelSizeState>
2642: pub fn persisted_panel_size_state(
2650: pub fn persist_panel_size_state(
2678: pub fn set_panel_size_state<T: Panel>(
2700: pub fn toggle_dock_panel_flexible_size(
2716: fn dock_size(&self, dock: &Dock, window: &Window, cx: &App) -> Option<Pixels>
2757: pub fn dock_flex_for_size(
2788: fn opposite_dock_panel_and_size_state(
2811: fn center_full_height_column_count(&self) -> f32
2815: pub fn default_dock_flex(&self, position: DockPosition) -> Option<f32>
2823: pub fn is_edited(&self) -> bool
2827: pub fn add_panel<T: Panel>(
2864: pub fn remove_panel<T: Panel>(
2875: pub fn status_bar(&self) -> &Entity<StatusBar>
2879: pub fn set_sidebar_focus_handle(&mut self, handle: Option<FocusHandle>)
2883: pub fn status_bar_visible(&self, cx: &App) -> bool
2887: pub fn multi_workspace(&self) -> Option<&WeakEntity<MultiWorkspace>>
2891: pub fn set_multi_workspace(
2904: pub fn app_state(&self) -> &Arc<AppState>
2908: pub fn is_restoring(&self) -> bool
2913: pub fn set_restoring_workspace(&mut self, restoring: bool)
2917: pub fn set_panels_task(&mut self, task: Task<Result<()>>)
2921: pub fn take_panels_task(&mut self) -> Option<Task<Result<()>>>
2925: pub fn user_store(&self) -> &Entity<UserStore>
2929: pub fn project(&self) -> &Entity<Project>
2933: pub fn path_style(&self, cx: &App) -> PathStyle
2937: pub fn recently_activated_items(&self, cx: &App) -> HashMap<EntityId, usize>
2958: pub fn recent_active_item_by_type<T: 'static>(&self, cx: &App) -> Option<Entity<T>>
2978: pub fn recent_navigation_history_iter(
3062: pub fn recent_navigation_history(
3072: pub fn clear_navigation_history(&mut self, window: &mut Window, cx: &mut Context<Workspace>)
3081: fn rename_persisted_navigation_history_paths(
3106: fn remember_navigation_history_path(&mut self, project_path: &ProjectPath, cx: &App) -> bool
3129: fn navigate_history(
3145: fn navigate_tag_history(
3161: fn navigate_history_impl(
3291: pub fn go_back(
3300: pub fn go_forward(
3309: pub fn reopen_closed_item(
3322: pub fn client(&self) -> &Arc<Client>
3326: pub fn set_titlebar_item(&mut self, item: AnyView, _: &mut Window, cx: &mut Context<Self>)
3331: pub fn set_prompt_for_new_path(&mut self, prompt: PromptForNewPath)
3335: pub fn set_prompt_for_open_path(&mut self, prompt: PromptForOpenPath)
3339: pub fn set_terminal_provider(&mut self, provider: impl TerminalProvider + 'static)
3343: pub fn set_debugger_provider(&mut self, provider: impl DebuggerProvider + 'static)
3347: pub fn set_open_in_dev_container(&mut self, value: bool)
3351: pub fn open_in_dev_container(&self) -> bool
3355: pub fn set_dev_container_task(&mut self, task: Task<Result<()>>)
3359: pub fn debugger_provider(&self) -> Option<Arc<dyn DebuggerProvider>>
3363: pub fn prompt_for_open_path(
3413: pub fn prompt_for_new_path(
3473: pub fn titlebar_item(&self) -> Option<AnyView>
3481: pub fn with_local_workspace<T, F>(
3521: pub fn with_local_or_wsl_workspace<T, F>(
3558: pub fn worktrees<'a>(&self, cx: &'a App) -> impl 'a + Iterator<Item = Entity<Worktree>>
3562: pub fn visible_worktrees<'a>(
3569: pub fn worktree_scans_complete(&self, cx: &App) -> impl Future<Output = ()> + 'static + use<>
3582: pub fn close_global(cx: &mut App)
3601: pub fn move_focused_panel_to_next_position(
3625: pub fn prepare_to_close(
3758: fn save_all(&mut self, action: &SaveAll, window: &mut Window, cx: &mut Context<Self>)
3768: fn send_keystrokes(
3788: pub fn send_keystrokes_impl(
3854: pub fn prompt_to_save_or_discard_dirty_items(
3862: fn save_all_internal(
3964: pub fn open_workspace_for_paths(
4009: pub fn open_paths(
4190: pub fn open_resolved_path(
4212: pub fn absolute_path_of_worktree(
4224: pub fn add_folder_to_project(
4271: pub fn project_path_for_path(
4287: pub fn items<'a>(&'a self, cx: &'a App) -> impl 'a + Iterator<Item = &'a Box<dyn ItemHandle>>
4291: pub fn item_of_type<T: Item>(&self, cx: &App) -> Option<Entity<T>>
4295: pub fn items_of_type<'a, T: Item>(
4304: pub fn active_item(&self, cx: &App) -> Option<Box<dyn ItemHandle>>
4308: pub fn active_item_as<I: 'static>(&self, cx: &App) -> Option<Entity<I>>
4320: fn active_project_path(&self, cx: &App) -> Option<ProjectPath>
4324: pub fn most_recent_active_path(&self, cx: &App) -> Option<PathBuf>
4347: pub fn save_active_item(
4368: pub fn close_inactive_items_and_panes(
4384: pub fn close_all_items_and_panes(
4401: pub fn close_item_in_all_panes(
4432: pub fn close_items_with_project_path(
4455: fn close_all_internal(
4513: pub fn is_dock_at_position_open(&self, position: DockPosition, cx: &mut Context<Self>) -> bool
4517: pub fn toggle_dock(
4583: fn active_dock(&self, window: &Window, cx: &Context<Self>) -> Option<&Entity<Dock>>
4589: fn close_active_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool
4600: pub fn close_all_docks(&mut self, window: &mut Window, cx: &mut Context<Self>)
4613: fn get_open_dock_positions(&self, cx: &Context<Self>) -> Vec<DockPosition>
4631: fn save_open_dock_positions(&mut self, cx: &mut Context<Self>)
4642: fn toggle_all_docks(
4661: fn restore_last_open_docks(&mut self, window: &mut Window, cx: &mut Context<Self>)
4675: pub fn focus_panel<T: Panel>(
4688: pub fn toggle_panel_focus<T: Panel>(
4712: pub fn focus_center_pane(&mut self, window: &mut Window, cx: &mut Context<Self>)
4720: pub fn activate_panel_for_proto_id(
4747: fn focus_or_unfocus_panel<T: Panel>(
4793: pub fn open_panel<T: Panel>(&mut self, window: &mut Window, cx: &mut Context<Self>)
4806: pub fn reveal_panel<T: Panel>(&mut self, window: &mut Window, cx: &mut Context<Self>)
4815: pub fn close_panel<T: Panel>(&self, window: &mut Window, cx: &mut Context<Self>)
4825: pub fn panel<T: Panel>(&self, cx: &App) -> Option<Entity<T>>
4834: pub fn fallback_focus_handle(&self, window: &Window, cx: &App) -> FocusHandle
4852: fn dismiss_zoomed_items_to_reveal(
4893: fn add_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Pane>
4918: pub fn add_item_to_center(
4938: pub fn add_item_to_active_pane(
4957: pub fn add_item(
4979: pub fn split_item(
4990: pub fn open_abs_path(
5021: pub fn split_abs_path(
5040: pub fn open_path(
5051: pub fn open_path_preview(
5110: fn pane_containing_project_item(
5166: pub fn open_url_or_file(
5257: pub fn split_path(
5266: pub fn split_path_preview(
5317: fn load_path(
5327: pub fn find_project_item<T>(
5354: pub fn is_project_item_open<T>(
5370: pub fn open_project_item<T>(
5456: pub fn open_shared_screen(
5471: pub fn auto_watch_state(&self) -> &AutoWatch
5475: fn next_watched_peer(&self, cx: &App) -> Option<PeerId>
5480: pub fn toggle_auto_watch(&mut self, window: &mut Window, cx: &mut Context<Self>)
5508: fn handle_auto_watch_video_tracks_changed(
5541: fn handle_auto_watch_local_share_stopped(
5558: pub fn activate_item(
5581: fn activate_pane_at_index(
5596: fn move_item_to_pane_at_index(
5647: pub fn activate_next_pane(&mut self, window: &mut Window, cx: &mut App)
5656: pub fn activate_previous_pane(&mut self, window: &mut Window, cx: &mut App)
5665: pub fn activate_last_pane(&mut self, window: &mut Window, cx: &mut App)
5670: pub fn activate_pane_in_direction(
5872: pub fn move_item_to_pane_in_direction(
5919: pub fn bounding_box_for_pane(&self, pane: &Entity<Pane>) -> Option<Bounds<Pixels>>
5923: pub fn find_pane_in_direction(
5933: pub fn swap_pane_in_direction(&mut self, direction: SplitDirection, cx: &mut Context<Self>)
5940: pub fn move_pane_to_border(&mut self, direction: SplitDirection, cx: &mut Context<Self>)
5950: pub fn resize_pane(
5979: pub fn reset_pane_sizes(&mut self, cx: &mut Context<Self>)
5984: fn handle_pane_focused(
6044: fn set_active_pane(
6055: fn handle_panel_focused(&mut self, window: &mut Window, cx: &mut Context<Self>)
6060: fn flush_deferred_saves(&mut self, window: &mut Window, cx: &mut Context<Self>)
6076: fn handle_pane_event(
6193: pub fn unfollow_in_pane(
6204: pub fn split_pane(
6218: pub fn split_and_move(
6236: pub fn split_and_clone(
6270: pub fn join_all_panes(&mut self, window: &mut Window, cx: &mut Context<Self>)
6281: pub fn join_pane_into_next(
6299: fn remove_pane(
6330: pub fn panes_mut(&mut self) -> &mut [Entity<Pane>]
6334: pub fn panes(&self) -> &[Entity<Pane>]
6338: pub fn active_pane(&self) -> &Entity<Pane>
6342: pub fn focused_pane(&self, window: &Window, cx: &App) -> Entity<Pane>
6356: pub fn adjacent_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Pane>
6360: pub fn adjacent_pane_of(
6372: pub fn pane_for(&self, handle: &dyn ItemHandle) -> Option<Entity<Pane>>
6376: pub fn pane_for_item_id(&self, item_id: EntityId) -> Option<Entity<Pane>>
6381: pub fn pane_for_entity_id(&self, entity_id: EntityId) -> Option<Entity<Pane>>
6388: fn collaborator_left(&mut self, peer_id: PeerId, window: &mut Window, cx: &mut Context<Self>)
6402: pub fn start_following(
6466: pub fn follow_next_collaborator(
6514: pub fn follow(
6577: pub fn unfollow(
6607: pub fn is_being_followed(&self, id: impl Into<CollaboratorId>) -> bool
6611: pub(crate) fn active_item_path_changed(
6655: fn owns_window_chrome(&self) -> bool
6662: fn update_window_title(&mut self, window: &mut Window, cx: &mut App)
6671: fn window_title_needs_branch(&self, cx: &App) -> bool
6676: fn apply_window_title(&mut self, window: &mut Window, cx: &mut App)
6735: fn is_window_edited(&self, cx: &App) -> bool
6739: fn window_title_context(
6822: fn update_window_edited(&mut self, window: &mut Window, cx: &mut App)
6849: pub fn refresh_window_state(&mut self, window: &mut Window, cx: &mut App)
6857: fn update_item_dirty_state(
6897: fn render_notifications(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Div>
6923: fn active_view_for_follower(
6958: fn handle_follow(
6973: fn handle_update_followers(
6985: async fn process_leader_update(
7058: async fn add_view_from_leader(
7155: fn handle_agent_location_changed(&mut self, window: &mut Window, cx: &mut Context<Self>)
7218: pub fn update_active_view_for_followers(&mut self, window: &mut Window, cx: &mut App)
7269: fn active_item_for_followers(
7294: fn update_followers(
7315: pub fn leader_for_pane(&self, pane: &Entity<Pane>) -> Option<CollaboratorId>
7325: fn leader_updated(
7372: fn active_item_for_agent(&self) -> Option<Box<dyn ItemHandle>>
7384: fn active_item_for_peer(
7424: fn shared_screen_for_peer(
7435: pub fn on_window_activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>)
7464: pub fn active_call(&self) -> Option<&dyn AnyActiveCall>
7468: pub fn active_global_call(&self) -> Option<GlobalAnyActiveCall>
7472: fn on_active_call_event(
7504: pub fn database_id(&self) -> Option<WorkspaceId>
7509: pub(crate) fn set_database_id(&mut self, id: WorkspaceId)
7513: pub fn session_id(&self) -> Option<String>
7517: fn save_window_bounds(&self, window: &mut Window, cx: &mut App) -> Task<()>
7562: pub fn flush_serialization(&mut self, window: &mut Window, cx: &mut App) -> Task<()>
7598: pub fn root_paths(&self, cx: &App) -> Vec<Arc<Path>>
7606: fn remove_panes(&mut self, member: Member, window: &mut Window, cx: &mut Context<Workspace>)
7619: fn remove_from_session(&mut self, window: &mut Window, cx: &mut App) -> Task<()>
7624: fn force_remove_pane(
7651: fn serialize_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>)
7668: fn serialize_workspace_internal(&self, window: &mut Window, cx: &mut App) -> Task<()>
7778: fn workspace_location(&self, cx: &App) -> WorkspaceLocation
7789: fn update_history(&self, cx: &mut App)
7804: async fn serialize_items(
7841: pub(crate) fn enqueue_item_serialization(
7850: pub(crate) fn load_workspace(
7997: pub fn key_context(&self, cx: &App) -> KeyContext
8047: pub fn actions(&self, div: Div, window: &mut Window, cx: &mut Context<Self>) -> Div
8446: pub fn set_random_database_id(&mut self)
8451: pub fn test_new(project: Entity<Project>, window: &mut Window, cx: &mut Context<Self>) -> Self
8477: pub fn register_action<A: Action>(
8491: pub fn register_action_renderer(
8499: fn add_workspace_actions_listeners(
8511: pub fn has_active_modal(&self, _: &mut Window, cx: &mut App) -> bool
8515: pub fn active_modal<V: ManagedView + 'static>(&self, cx: &App) -> Option<Entity<V>>
8526: pub fn toggle_modal<V: ModalView, B>(&mut self, window: &mut Window, cx: &mut App, build: B)
8535: pub fn hide_modal(&mut self, window: &mut Window, cx: &mut App) -> bool
8540: fn reopen_last_picker(
8556: pub fn toggle_status_toast<V: ToastView>(&mut self, entity: Entity<V>, cx: &mut App)
8561: pub fn toggle_centered_layout(
8579: pub fn clear_bookmarks(&mut self, _: &ClearBookmarks, _: &mut Window, cx: &mut Context<Self>)
8588: pub fn toggle_editor_zoom(
8615: pub fn is_pane_maximized(&self) -> bool
8619: fn adjust_padding(padding: Option<f32>) -> f32
8628: fn render_dock(
8728: fn focusable_parts(&self, cx: &App) -> Vec<FocusablePart>
8784: fn move_part_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>)
8835: fn move_titlebar_item_focus(
8865: fn render_center(
8888: pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>>
8895: pub fn zoomed_item(&self) -> Option<&AnyWeakView>
8899: pub fn activate_next_window(&mut self, cx: &mut Context<Self>)
8922: pub fn activate_previous_window(&mut self, cx: &mut Context<Self>)
8946: pub fn cancel(&mut self, _: &menu::Cancel, window: &mut Window, cx: &mut Context<Self>)
8955: fn resize_dock(
8969: fn resize_left_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App)
8988: fn resize_right_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App)
9005: fn resize_bottom_dock(&mut self, new_size: Pixels, window: &mut Window, cx: &mut App)
9012: fn toggle_edit_predictions_all_files(
9025: fn toggle_theme_mode(&mut self, _: &ToggleMode, _window: &mut Window, cx: &mut Context<Self>)
9048: pub fn show_worktree_trust_security_modal(
9083: fn project_window_title(project: &Project, cx: &App) -> String
9102: pub trait AnyActiveCall
9103: fn entity(&self) -> AnyEntity
9104: fn is_in_room(&self, _: &App) -> bool
9105: fn room_id(&self, _: &App) -> Option<u64>
9106: fn channel_id(&self, _: &App) -> Option<ChannelId>
9107: fn hang_up(&self, _: &mut App) -> Task<Result<()>>
9108: fn unshare_project(&self, _: Entity<Project>, _: &mut App) -> Result<()>
9109: fn remote_participant_for_peer_id(&self, _: PeerId, _: &App) -> Option<RemoteCollaborator>
9110: fn is_sharing_project(&self, _: &App) -> bool
9111: fn is_sharing_screen(&self, _: &App) -> bool
9112: fn has_remote_participants(&self, _: &App) -> bool
9113: fn local_participant_is_guest(&self, _: &App) -> bool
9114: fn client(&self, _: &App) -> Arc<Client>
9115: fn share_on_join(&self, _: &App) -> bool
9116: fn join_channel(&self, _: ChannelId, _: &mut App) -> Task<Result<bool>>
9117: fn room_update_completed(&self, _: &mut App) -> Task<()>
9118: fn most_active_project(&self, _: &App) -> Option<(u64, u64)>
9119: fn share_project(&self, _: Entity<Project>, _: &mut App) -> Task<Result<u64>>
9120: fn join_project(
9127: fn peer_id_for_user_in_room(&self, _: u64, _: &App) -> Option<PeerId>
9128: fn subscribe(
9134: fn create_shared_screen(
9141: fn peer_ids_with_video_tracks(&self, _: &App) -> Vec<PeerId>
9145: pub struct GlobalAnyActiveCall(pub Arc<dyn AnyActiveCall>)
9146: impl Global for GlobalAnyActiveCall
9148: impl GlobalAnyActiveCall
9149: pub(crate) fn try_global(cx: &App) -> Option<&Self>
9153: pub(crate) fn global(cx: &App) -> &Self
9160: pub enum ParticipantLocation
9161: SharedProject
9162: UnsharedProject
9163: External
9166: impl ParticipantLocation
9167: pub fn from_proto(location: Option<proto::ParticipantLocation>) -> Result<Self>
9185: pub struct RemoteCollaborator
9186: pub user: Arc<User>
9187: pub peer_id: PeerId
9188: pub location: ParticipantLocation
9189: pub participant_index: ParticipantIndex
9192: pub enum ActiveCallEvent
9193: ParticipantLocationChanged
9194: RemoteVideoTracksChanged
9195: LocalScreenShareStarted
9196: LocalScreenShareStopped
9197: RoomLeft
9200: fn leader_border_for_pane(
9239: fn window_bounds_env_override() -> Option<Bounds<Pixels>>
9248: fn open_items(
9354: enum ActivateInDirectionTarget
9355: Pane
9356: Dock
9357: Sidebar
9362: struct FocusablePart
9367: container: FocusHandle
9368: behavior: PartBehavior
9371: enum PartBehavior
9375: Toolbar
9381: Landmark
9384: impl FocusablePart
9385: fn toolbar(container: FocusHandle) -> Self
9392: fn landmark(wrapper: FocusHandle, content: FocusHandle) -> Self
9403: fn contains_focused(&self, window: &Window, cx: &App) -> bool
9420: struct RegionFocusHandles
9421: left_dock: FocusHandle
9422: right_dock: FocusHandle
9423: bottom_dock: FocusHandle
9424: editor: FocusHandle
9427: impl RegionFocusHandles
9428: fn new(cx: &mut App) -> Self
9437: fn dock(&self, position: DockPosition) -> &FocusHandle
9446: fn notify_if_database_failed(window: WindowHandle<MultiWorkspace>, cx: &mut AsyncApp)
9474: fn px_with_ui_font_fallback(val: u32, cx: &Context<Workspace>) -> Pixels
9482: fn adjust_active_dock_size_by_px(
9502: fn adjust_open_docks_size_by_px(
9528: impl Focusable for Workspace
9529: fn focus_handle(&self, cx: &App) -> FocusHandle
9535: struct DraggedDock(DockPosition)
9537: impl Render for DraggedDock
9538: fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement
9543: impl Render for Workspace
9544: fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement
10017: impl WorkspaceStore
10018: pub fn new(client: Arc<Client>, cx: &mut Context<Self>) -> Self
10029: pub fn update_followers(
10046: pub async fn handle_follow(
10082: async fn handle_update_followers(
10116: pub fn workspaces(&self) -> impl Iterator<Item = &WeakEntity<Workspace>>
10120: pub fn workspaces_with_windows(
10127: impl ViewId
10128: pub(crate) fn from_proto(message: proto::ViewId) -> Result<Self>
10138: pub(crate) fn to_proto(self) -> Option<proto::ViewId>
10150: impl FollowerState
10151: fn pane(&self) -> &Entity<Pane>
10156: pub trait WorkspaceHandle
10157: fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath>
10160: impl WorkspaceHandle for Entity<Workspace>
10161: fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath>
10175: pub async fn last_opened_workspace_location(
10186: pub async fn last_session_workspace_locations(
10197: pub async fn restore_multiworkspace(
10284: pub async fn apply_restored_multiworkspace_state(
10355: fn restore_native_window_state(
10417: pub struct OpenChannelNotesById
10418: pub channel_id: u64
10431: async fn join_channel_internal(
10572: fn serialize_pane_handle(
10612: pub fn join_channel(
10717: pub async fn get_any_active_multi_workspace(
10740: pub fn activate_any_workspace_window(cx: &mut AsyncApp) -> Option<WindowHandle<MultiWorkspace>>
10761: pub fn workspace_windows_for_location(
10811: pub async fn find_existing_workspace(
10902: pub enum WorkspaceMatching
10904: None
10907: MatchExact
10910: MatchSubpaths
10916: MatchSubdirectory
10920: pub struct OpenOptions
10921: pub visible: Option<OpenVisible>
10922: pub focus: Option<bool>
10923: pub workspace_matching: WorkspaceMatching
10927: pub add_dirs_to_sidebar: bool
10928: pub wait: bool
10929: pub requesting_window: Option<WindowHandle<MultiWorkspace>>
10930: pub open_mode: OpenMode
10931: pub env: Option<HashMap<String, String>>
10932: pub open_in_dev_container: bool
10935: impl Default for OpenOptions
10936: fn default() -> Self
10951: impl OpenOptions
10952: fn should_reuse_existing_window(&self) -> bool
10962: pub struct OpenResult
10963: pub window: WindowHandle<MultiWorkspace>
10964: pub workspace: Entity<Workspace>
10965: pub opened_items: Vec<Option<anyhow::Result<Box<dyn ItemHandle>>>>
10969: pub fn open_workspace_by_id(
11084: pub fn open_paths(
11283: pub fn open_new(
11310: pub fn create_and_open_local_file(
11359: pub fn open_remote_project_with_new_connection(
11418: pub fn open_remote_project_with_existing_connection(
11447: async fn open_remote_project_inner(
11562: fn deserialize_remote_project(
11587: pub fn join_in_room_project(
11681: pub fn reload(cx: &mut App)
11725: pub async fn prepare_windows_to_quit(
11769: pub(crate) async fn prepare_window_to_close(
11825: pub async fn flush_windows_serialization(
11833: fn flush_windows_serialization_on_quit(cx: &mut App) -> impl Future<Output = ()> + use<>
11845: fn collect_flush_tasks(
11866: fn parse_pixel_position_env_var(value: &str) -> Option<Point<Pixels>>
11873: fn parse_pixel_size_env_var(value: &str) -> Option<Size<Pixels>>
11882: pub fn client_side_decorations(
12039: fn resize_edge(
12096: fn join_pane_into_active(
12114: fn move_all_items(
12146: pub fn move_item(
12190: pub fn move_active_item(
12220: pub fn clone_active_item(
12261: pub struct WorkspacePosition
12262: pub window_bounds: Option<WindowBounds>
12263: pub display: Option<Uuid>
12264: pub centered_layout: bool
12267: pub fn remote_workspace_position_from_db(
12313: pub fn with_active_or_new_workspace(
12346: fn load_legacy_panel_size(
```
