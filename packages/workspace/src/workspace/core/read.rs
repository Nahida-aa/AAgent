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
    pub fn root_paths(&self, cx: &App) -> Vec<Arc<Path>> {
        let project = self.project().read(cx);
        project
            .visible_worktrees(cx)
            .map(|worktree| worktree.read(cx).abs_path())
            .collect::<Vec<_>>()
    }

    // │   ├── key_context
    pub fn key_context(&self, cx: &App) -> KeyContext {
        let mut context = KeyContext::new_with_defaults();
        context.add("Workspace");
        context.set("keyboard_layout", cx.keyboard_layout().name().to_string());
        if let Some(status) = self
            .debugger_provider
            .as_ref()
            .and_then(|provider| provider.active_thread_state(cx))
        {
            match status {
                ThreadStatus::Running | ThreadStatus::Stepping => {
                    context.add("debugger_running");
                }
                ThreadStatus::Stopped => context.add("debugger_stopped"),
                ThreadStatus::Exited | ThreadStatus::Ended => {}
            }
            // A coarse "there is a live debug session" flag (running, stepping,
            // or stopped at a breakpoint) used to gate debugger controls like
            // step/pause/stop. Distinct from `debugger_running`, which is only
            // true while the program is actually executing.
            if matches!(
                status,
                ThreadStatus::Running | ThreadStatus::Stepping | ThreadStatus::Stopped
            ) {
                context.add("debugger_session");
            }
        }

        if self.left_dock.read(cx).is_open() {
            if let Some(active_panel) = self.left_dock.read(cx).active_panel() {
                context.set("left_dock", active_panel.panel_key());
            }
        }

        if self.right_dock.read(cx).is_open() {
            if let Some(active_panel) = self.right_dock.read(cx).active_panel() {
                context.set("right_dock", active_panel.panel_key());
            }
        }

        if self.bottom_dock.read(cx).is_open() {
            if let Some(active_panel) = self.bottom_dock.read(cx).active_panel() {
                context.set("bottom_dock", active_panel.panel_key());
            }
        }

        context
    }

    // │   ├── for_window
    pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>> {
        window
            .root::<MultiWorkspace>()
            .flatten()
            .map(|multi_workspace| multi_workspace.read(cx).workspace().clone())
    }
    // │   └── zoomed_item
    pub fn zoomed_item(&self) -> Option<&AnyWeakView> {
        self.zoomed.as_ref()
    }

}
