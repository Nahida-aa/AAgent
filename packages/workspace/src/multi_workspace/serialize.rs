use futures::Future;
use gpui::{App, Context, Task, Window};

use super::MultiWorkspace;
use crate::persistence::model::MultiWorkspaceState;

impl MultiWorkspace {
    pub fn serialize(&mut self, cx: &mut Context<Self>) {
        self._serialize_task = Some(cx.spawn(async move |this, cx| {
            let Ok(task) = this.update(cx, |this, cx| this.serialize_now(cx)) else {
                return;
            };
            task.await;
        }));
    }

    fn serialize_now(&mut self, cx: &mut Context<Self>) -> impl Future<Output = ()> + use<> {
        let state = MultiWorkspaceState {
            active_workspace_id: self.workspace().read(cx).database_id(),
            project_groups: self
                .project_groups
                .iter()
                .map(|group| {
                    crate::persistence::model::SerializedProjectGroup::from_group(
                        &group.key,
                        group.expanded,
                    )
                })
                .collect::<Vec<_>>(),
            sidebar_open: self.sidebar_open,
            sidebar_state: self.sidebar.as_ref().and_then(|s| s.serialized_state(cx)),
        };
        let window_id = self.window_id;
        let kvp = db::kvp::KeyValueStore::global(cx);
        async move {
            crate::persistence::write_multi_workspace_state(&kvp, window_id, state).await;
        }
    }

    /// Used by the quit handler to ensure pending DB writes
    /// complete before the process exits.
    pub fn flush_serialization(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self._serialize_task.take();
        let serialization = self.serialize_now(cx);
        cx.spawn(async move |_, _| serialization.await)
    }

    pub fn flush_pending_serialization(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Task<()>> {
        let mut tasks = Vec::new();
        for workspace in self.workspaces() {
            tasks.push(workspace.update(cx, |workspace, cx| {
                workspace.flush_serialization(window, cx)
            }));
        }
        tasks.append(&mut self.take_pending_removal_tasks());
        tasks.push(self.flush_serialization(cx));
        tasks
    }

    pub fn take_pending_removal_tasks(&mut self) -> Vec<Task<()>> {
        let tasks: Vec<Task<()>> = std::mem::take(&mut self.pending_removal_tasks)
            .into_iter()
            .filter(|task| !task.is_ready())
            .collect();
        tasks
    }
}
