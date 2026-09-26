use super::*;
impl Workspace {
    // project_group_key          // 转发 project.project_group_key
    pub fn project_group_key(&self, cx: &App) -> ProjectGroupKey {
        self.project.read(cx).project_group_key(cx)
    }
    // weak_handle
    pub fn weak_handle(&self) -> WeakEntity<Self> { self.weak_self.clone() }
    // multi_workspace
    pub fn multi_workspace(&self) -> Option<&WeakEntity<MultiWorkspace>> {
        self.multi_workspace.as_ref()
    }
    // app_state
    pub fn app_state(&self) -> &Arc<AppState> { &self.app_state }
    // is_restoring
    pub fn is_restoring(&self) -> bool { self.restoring_workspace }
    // take_panels_task
    pub fn take_panels_task(&mut self) -> Option<Task<Result<()>>> { self._panels_task.take() }
    // user_store
    pub fn user_store(&self) -> &Entity<UserStore> { &self.app_state.user_store }
    // project
    pub fn project(&self) -> &Entity<Project> { &self.project }
    // path_style
    pub fn path_style(&self, cx: &App) -> PathStyle { self.project.read(cx).path_style(cx) }
    //
    pub fn client(&self) -> &Arc<Client> { &self.app_state.client }
    //
    pub fn open_in_dev_container(&self) -> bool { self.open_in_dev_container }
    // │   ├── database_id
    pub fn database_id(&self) -> Option<WorkspaceId> { self.database_id }
    // │   ├── session_id
    pub fn session_id(&self) -> Option<String> { self.session_id.clone() }
    // │   ├── root_paths
    pub fn root_paths(&self, cx: &App) -> Vec<Arc<Path>>;
    // │   ├── key_context
    pub fn key_context(&self, cx: &App) -> KeyContext;
    // │   ├── for_window
    pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>>;
    // │   └── zoomed_item
    pub fn zoomed_item(&self) -> Option<&AnyWeakView>;
}
