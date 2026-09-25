pub trait DebuggerProvider {
    // `active_buffer` is used to resolve build task's name against language-specific tasks.
    fn start_session(
        &self,
        definition: DebugScenario,
        task_context: SharedTaskContext,
        active_buffer: Option<Entity<Buffer>>,
        worktree_id: Option<WorktreeId>,
        window: &mut Window,
        cx: &mut App,
    );

    fn spawn_task_or_modal(
        &self,
        workspace: &mut Workspace,
        action: &Spawn,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    );

    fn task_scheduled(&self, cx: &mut App);
    fn debug_scenario_scheduled(&self, cx: &mut App);
    fn debug_scenario_scheduled_last(&self, cx: &App) -> bool;

    fn active_thread_state(&self, cx: &App) -> Option<ThreadStatus>;
}

impl Workspace {
    //
    pub fn set_debugger_provider(&mut self, provider: impl DebuggerProvider + 'static) {
        self.debugger_provider = Some(Arc::new(provider));
    }
    pub fn debugger_provider(&self) -> Option<Arc<dyn DebuggerProvider>> {
        self.debugger_provider.clone()
    }

}
