use futures::{
    Future, FutureExt, StreamExt,
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{Shared, try_join_all},
};
use gpui::{App, AppContext, WindowHandle};

use crate::MultiWorkspace;

impl Workspace {
    // ├── flush_serialization
    /// Bypass the serialization throttles and write workspace and item state
    /// to the DB immediately. Returns a task the caller can await to ensure the
    /// writes complete before the process exits.
    pub fn flush_serialization(&mut self, window: &mut Window, cx: &mut App) -> Task<()> {
        self._schedule_serialize_workspace.take();
        self._serialize_workspace_task.take();
        self.bounds_save_task_queued.take();

        let serializable_items = self
            .panes
            .iter()
            .flat_map(|pane| pane.read(cx).items())
            .filter_map(|item| item.to_serializable_item_handle(cx))
            .fold(HashMap::default(), |mut items, item| {
                items.entry(item.item_id()).or_insert(item);
                items
            });
        let item_tasks = serializable_items
            .into_values()
            .filter_map(|item| {
                let item_id = item.item_id();
                let task = item.serialize(self, false, cx)?;
                Some(async move {
                    task.await
                        .with_context(|| format!("flushing serialization of item {item_id:?}"))
                })
            })
            .collect::<Vec<_>>();
        let bounds_task = self.save_window_bounds(window, cx);
        let serialize_task = self.serialize_workspace_internal(window, cx);
        cx.background_spawn(async move {
            bounds_task.await;
            serialize_task.await;
            for result in futures::future::join_all(item_tasks).await {
                result.log_err();
            }
        })
    }

    // ├── flush_windows_serialization
    // ├── flush_windows_serialization_on_quit
    // └── collect_flush_tasks
}

pub async fn flush_windows_serialization(
    workspace_windows: &[WindowHandle<MultiWorkspace>],
    cx: &mut AsyncApp,
) {
    let flush_tasks = collect_flush_tasks(workspace_windows, cx);
    futures::future::join_all(flush_tasks).await;
}

pub(crate) fn flush_windows_serialization_on_quit(
    cx: &mut App,
) -> impl Future<Output = ()> + use<> {
    let workspace_windows = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<MultiWorkspace>())
        .collect::<Vec<_>>();
    let flush_tasks = collect_flush_tasks(&workspace_windows, cx);
    async move {
        futures::future::join_all(flush_tasks).await;
    }
}

fn collect_flush_tasks(
    workspace_windows: &[WindowHandle<MultiWorkspace>],
    cx: &mut impl AppContext,
) -> Vec<Task<()>> {
    let mut flush_tasks = Vec::new();
    for window in workspace_windows {
        window
            .update(cx, |multi_workspace, window, cx| {
                flush_tasks.extend(multi_workspace.flush_pending_serialization(window, cx));
            })
            .with_context(|| {
                format!(
                    "flushing pending serialization for window {:?}",
                    window.window_id()
                )
            })
            .log_err();
    }
    flush_tasks
}
