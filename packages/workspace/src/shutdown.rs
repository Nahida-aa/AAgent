use gpui::{App, AppContext, AsyncApp, Task, WindowHandle};

use crate::multi_workspace::MultiWorkspace;
use crate::startup::window_utils::workspace_windows_for_location;
use crate::types::CloseIntent;

pub fn reload(cx: &mut App) {
    let should_confirm = WorkspaceSettings::get_global(cx).confirm_quit;
    let mut workspace_windows = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<MultiWorkspace>())
        .collect::<Vec<_>>();

    // If multiple windows have unsaved changes, and need a save prompt,
    // prompt in the active window before switching to a different window.
    workspace_windows.sort_by_key(|window| window.is_active(cx) == Some(false));

    let mut prompt = None;
    if let (true, Some(window)) = (should_confirm, workspace_windows.first()) {
        prompt = window
            .update(cx, |_, window, cx| {
                window.prompt(
                    PromptLevel::Info,
                    "Are you sure you want to restart?",
                    None,
                    &["Restart", "Cancel"],
                    cx,
                )
            })
            .ok();
    }

    cx.spawn(async move |cx| {
        if let Some(prompt) = prompt {
            let answer = prompt.await?;
            if answer != 0 {
                return anyhow::Ok(());
            }
        }

        if !prepare_windows_to_quit(&workspace_windows, cx).await {
            return anyhow::Ok(());
        }
        cx.update(|cx| cx.restart());
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

pub async fn prepare_windows_to_quit(
    workspace_windows: &[WindowHandle<MultiWorkspace>],
    cx: &mut AsyncApp,
) -> bool {
    // If the user cancels any save prompt, then keep the app open.
    let mut prepared_windows = Vec::new();
    let mut cancelled = false;
    for window in workspace_windows {
        match prepare_window_to_close(*window, CloseIntent::Quit, cx).await {
            Ok(true) => prepared_windows.push(*window),
            Ok(false) => {
                cancelled = true;
                break;
            }
            Err(error) => {
                log::error!(
                    "failed to prepare window {:?} to close before quitting: {error:#}",
                    window.window_id()
                );
                cancelled = true;
                break;
            }
        }
    }

    if cancelled {
        flush_windows_serialization(&prepared_windows, cx).await;
        for window in prepared_windows {
            window
                .update(cx, |_, window, _cx| {
                    window.remove_window();
                })
                .log_err();
        }
        return false;
    }

    // Flush all pending workspace serialization before quitting so that
    // session_id/window_id are up-to-date in the database.
    flush_windows_serialization(workspace_windows, cx).await;

    true
}

pub(crate) async fn prepare_window_to_close(
    window: WindowHandle<MultiWorkspace>,
    close_intent: CloseIntent,
    cx: &mut AsyncApp,
) -> Result<bool> {
    let active_and_workspaces = window
        .update(cx, |multi_workspace, window, _cx| {
            if close_intent == CloseIntent::Quit {
                window.activate_window();
            }
            (
                multi_workspace.workspace().clone(),
                multi_workspace.workspaces().cloned().collect::<Vec<_>>(),
            )
        })
        .log_err();

    let Some((originally_active, workspaces)) = active_and_workspaces else {
        return Ok(true);
    };

    let mut prepared = anyhow::Ok(true);
    for workspace in workspaces {
        prepared = match window.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.prepare_to_close(close_intent, window, cx)
            })
        }) {
            Ok(task) => task.await,
            Err(error) => Err(error),
        }
        .with_context(|| format!("preparing workspace {:?} to close", workspace.entity_id()));
        if !matches!(prepared, Ok(true)) {
            break;
        }
    }

    // Re-activate the workspace the user actually had focused so it is the
    // one serialized (and restored on next launch) as active, rather than
    // whichever happened to be last.
    window
        .update(cx, |multi_workspace, window, cx| {
            if !matches!(prepared, Ok(true)) {
                for workspace in multi_workspace.workspaces() {
                    workspace.update(cx, |workspace, _| {
                        workspace.removing = false;
                    });
                }
            }
            multi_workspace.activate(originally_active, None, window, cx);
        })
        .log_err();

    prepared
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
