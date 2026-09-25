#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseIntent {
    /// Quit the program entirely.
    Quit,
    /// Close a window.
    CloseWindow,
    /// Replace the workspace in an existing window.
    ReplaceWindow,
}

impl Workspace {
    // prepare_to_close
    pub fn prepare_to_close(
        &mut self,
        close_intent: CloseIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        let active_call = self.active_global_call();

        cx.spawn_in(window, async move |this, cx| {
            this.update(cx, |this, _| {
                if close_intent == CloseIntent::CloseWindow {
                    this.removing = true;
                }
            })?;

            let workspace_count = cx.update(|_window, cx| {
                cx.windows()
                    .iter()
                    .filter(|window| window.downcast::<MultiWorkspace>().is_some())
                    .count()
            })?;

            let (remaining_workspaces, closing_last_window_quits) = cx.update(|window, cx| {
                let current_window = window.window_handle();
                let remaining_workspaces =
                    cx.windows()
                        .into_iter()
                        .filter(|window| *window != current_window)
                        .filter_map(|window| window.downcast::<MultiWorkspace>())
                        .filter_map(|multi_workspace| {
                            multi_workspace.read(cx).ok().map(|multi_workspace| {
                                multi_workspace.workspace().read(cx).removing
                            })
                        })
                        .filter(|removing| !removing)
                        .count();

                (remaining_workspaces, closing_last_window_quits_app(cx))
            })?;

            let save_last_workspace = close_intent != CloseIntent::ReplaceWindow
                && remaining_workspaces == 0
                && closing_last_window_quits;

            if let Some(active_call) = active_call
                && workspace_count == 1
                && cx
                    .update(|_window, cx| active_call.0.is_in_room(cx))
                    .unwrap_or(false)
            {
                if close_intent == CloseIntent::CloseWindow {
                    this.update(cx, |_, cx| cx.emit(Event::Activate))?;
                    let answer = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Warning,
                            "Do you want to leave the current call?",
                            None,
                            &["Close window and hang up", "Cancel"],
                            cx,
                        )
                    })?;

                    if answer.await.log_err() == Some(1) {
                        return anyhow::Ok(false);
                    } else {
                        if let Ok(task) = cx.update(|_window, cx| active_call.0.hang_up(cx)) {
                            task.await.log_err();
                        }
                    }
                }
                if close_intent == CloseIntent::ReplaceWindow {
                    _ = cx.update(|_window, cx| {
                        let multi_workspace = cx
                            .windows()
                            .iter()
                            .filter_map(|window| window.downcast::<MultiWorkspace>())
                            .next()
                            .unwrap();
                        let project = multi_workspace
                            .read(cx)?
                            .workspace()
                            .read(cx)
                            .project
                            .clone();
                        if project.read(cx).is_shared() {
                            active_call.0.unshare_project(project, cx)?;
                        }
                        Ok::<_, anyhow::Error>(())
                    });
                }
            }

            // Hot-exit silently writes dirty buffers to the DB; only allow it
            // if the workspace will be reachable again, either via session
            // restore or by reopening its folder paths. Otherwise prompt, so
            // we don't orphan the buffers.
            let allow_hot_exit_serialization = close_intent == CloseIntent::Quit
                || save_last_workspace
                || this
                    .read_with(cx, |workspace, cx| {
                        workspace
                            .project
                            .read(cx)
                            .visible_worktrees(cx)
                            .next()
                            .is_some()
                    })
                    .unwrap_or(false);
            let save_result = this
                .update_in(cx, |this, window, cx| {
                    this.save_all_internal(
                        SaveIntent::Close,
                        allow_hot_exit_serialization,
                        window,
                        cx,
                    )
                })?
                .await;

            // If we're not quitting, but closing, we remove the workspace from
            // the current session.
            if close_intent != CloseIntent::Quit
                && !save_last_workspace
                && save_result.as_ref().is_ok_and(|&res| res)
            {
                this.update_in(cx, |this, window, cx| this.remove_from_session(window, cx))?
                    .await;
            }

            save_result
        })
    }
    // / reload / flush

}

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
