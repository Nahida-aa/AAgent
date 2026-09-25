impl Workspace {
    // 子实体与句柄读取
    pub fn weak_handle(&self) -> WeakEntity<Self>;
    pub fn app_state(&self) -> &Arc<AppState>;
    pub fn project(&self) -> &Entity<Project>;
    pub fn user_store(&self) -> &Entity<UserStore>;
    pub fn client(&self) -> &Arc<Client>;

    pub fn left_dock(&self) -> &Entity<Dock>;
    pub fn right_dock(&self) -> &Entity<Dock>;
    pub fn bottom_dock(&self) -> &Entity<Dock>;
    pub fn all_docks(&self) -> [&Entity<Dock>; 3];
    pub fn dock_at_position(&self, position: DockPosition) -> &Entity<Dock>;

    pub fn status_bar(&self) -> &Entity<StatusBar>;
    pub fn titlebar_item(&self) -> Option<AnyView>;
    pub fn zoomed_item(&self) -> Option<&AnyWeakView>;

    pub fn multi_workspace(&self) -> Option<&WeakEntity<MultiWorkspace>>;
    pub fn debugger_provider(&self) -> Option<Arc<dyn DebuggerProvider>>;

    //  pane 与 item 的读取
    pub fn panes(&self) -> &[Entity<Pane>];
    pub fn panes_mut(&mut self) -> &mut [Entity<Pane>];
    pub fn active_pane(&self) -> &Entity<Pane>;
    pub fn focused_pane(&self, window: &Window, cx: &App) -> Entity<Pane>;

    pub fn pane_for(&self, handle: &dyn ItemHandle) -> Option<Entity<Pane>>;
    pub fn pane_for_item_id(&self, item_id: EntityId) -> Option<Entity<Pane>>;
    pub fn pane_for_entity_id(&self, entity_id: EntityId) -> Option<Entity<Pane>>;

    pub fn active_item(&self, cx: &App) -> Option<Box<dyn ItemHandle>>;
    pub fn active_item_as<I: 'static>(&self, cx: &App) -> Option<Entity<I>>;
    pub fn items<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = &'a Box<dyn ItemHandle>>;
    pub fn item_of_type<T: Item>(&self, cx: &App) -> Option<Entity<T>>;
    pub fn items_of_type<'a, T: Item>(&'a self, cx: &'a App) -> impl Iterator<Item = Entity<T>>;

    pub fn worktrees<'a>(&self, cx: &'a App) -> impl Iterator<Item = Entity<Worktree>>;
    pub fn visible_worktrees<'a>(&self, cx: &'a App) -> impl Iterator<Item = Entity<Worktree>>;
    pub fn root_paths(&self, cx: &App) -> Vec<Arc<Path>>;

    pub fn panel<T: Panel>(&self, cx: &App) -> Option<Entity<T>>;
    pub fn panel_size_state<T: Panel>(&self, cx: &App) -> Option<dock::PanelSizeState>;
    pub fn persisted_panel_size_state(&self, panel_key: &'static str, cx: &App) -> Option<dock::PanelSizeState>;

    // 简单状态查询
    pub fn database_id(&self) -> Option<WorkspaceId>;
    pub fn session_id(&self) -> Option<String>;
    pub fn is_restoring(&self) -> bool;
    pub fn is_edited(&self) -> bool;
    pub fn is_pane_maximized(&self) -> bool;
    pub fn is_dock_at_position_open(&self, position: DockPosition, cx: &mut Context<Self>) -> bool;
    pub fn status_bar_visible(&self, cx: &App) -> bool;
    pub fn open_in_dev_container(&self) -> bool;
    pub fn active_worktree_creation(&self) -> &ActiveWorktreeCreation;

    pub fn path_style(&self, cx: &App) -> PathStyle;
    pub fn project_group_key(&self, cx: &App) -> ProjectGroupKey;
    pub fn focused_dock_position(&self, window: &Window, cx: &App) -> Option<DockPosition>;
    pub fn agent_panel_position(&self, cx: &App) -> Option<DockPosition>;

    pub fn active_call(&self) -> Option<&dyn AnyActiveCall>;
    pub fn active_global_call(&self) -> Option<GlobalAnyActiveCall>;
    pub fn is_being_followed(&self, id: impl Into<CollaboratorId>) -> bool;
    pub fn leader_for_pane(&self, pane: &Entity<Pane>) -> Option<CollaboratorId>;

    // 关联查询（不改状态但需要 window/cx）
    pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>>;
    pub fn has_active_modal(&self, window: &mut Window, cx: &mut App) -> bool;
    pub fn active_modal<V: ManagedView + 'static>(&self, cx: &App) -> Option<Entity<V>>;
    pub fn key_context(&self, cx: &App) -> KeyContext;
}
