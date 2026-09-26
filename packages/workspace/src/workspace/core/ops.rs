use super::*;
impl Workspace {
    // set_sidebar_focus_handle   // setter，但只是字段写，可放这里
    pub fn set_sidebar_focus_handle(&mut self, handle: Option<FocusHandle>) {
        self.sidebar_focus_handle = handle;
    }
    // set_multi_workspace        // 写字段 + 更新 status_bar
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
    //
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_restoring_workspace(&mut self, restoring: bool) {
        self.restoring_workspace = restoring;
    }
    // set_panels_task
    pub fn set_panels_task(&mut self, task: Task<Result<()>>) {
        self._panels_task = Some(task);
    }
    //
    pub fn set_open_in_dev_container(&mut self, value: bool) {
        self.open_in_dev_container = value;
    }
    // pub fn set_dev_container_task
    pub fn set_dev_container_task(&mut self, task: Task<Result<()>>) {
        self._dev_container_task = Some(task);
    }
    // │   ├── set_database_id        // test
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_database_id(&mut self, id: WorkspaceId) {
        self.database_id = Some(id);
    }
    // │   ├── set_random_database_id // test
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_random_database_id(&mut self) {
        self.database_id = Some(WorkspaceId(Uuid::new_v4().as_u64_pair().0 as i64));
    }
    // │   ├── toggle_centered_layout
    pub fn toggle_centered_layout(
        &mut self,
        _: &ToggleCenteredLayout,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.centered_layout = !self.centered_layout;
        if let Some(database_id) = self.database_id() {
            let db = WorkspaceDb::global(cx);
            let centered_layout = self.centered_layout;
            cx.background_spawn(async move {
                db.set_centered_layout(database_id, centered_layout).await
            })
            .detach_and_log_err(cx);
        }
        cx.notify();
    }
    // │   ├── clear_bookmarks
    pub fn clear_bookmarks(&mut self, _: &ClearBookmarks, _: &mut Window, cx: &mut Context<Self>) {
        self.project()
            .read(cx)
            .bookmark_store()
            .update(cx, |bookmark_store, cx| {
                bookmark_store.clear_bookmarks(cx);
            });
    }
    // │   ├── cancel
    pub fn cancel(&mut self, _: &menu::Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if cx.stop_active_drag(window) {
        } else if let Some((notification_id, _)) = self.notifications.pop() {
            dismiss_app_notification(&notification_id, cx);
        } else {
            cx.propagate();
        }
    }
    // │   ├── toggle_edit_predictions_all_files
    // │   └── toggle_theme_mode
}
