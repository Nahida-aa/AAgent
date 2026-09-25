use super::Workspace;
use crate::{dock::Dock, workspace::event::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
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

    pub(crate) fn save_all(
        &mut self,
        action: &SaveAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_all_internal(
            action.save_intent.unwrap_or(SaveIntent::SaveAll),
            true,
            window,
            cx,
        )
        .detach_and_log_err(cx);
    }
    fn save_all_internal(
        &mut self,
        mut save_intent: SaveIntent,
        allow_hot_exit_serialization: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        if self.project.read(cx).is_disconnected(cx) {
            return Task::ready(Ok(true));
        }
        let dirty_items = self
            .panes
            .iter()
            .flat_map(|pane| {
                pane.read(cx).items().filter_map(|item| {
                    if item.is_dirty(cx) {
                        item.tab_content_text(0, cx);
                        Some((pane.clone(), item.boxed_clone()))
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();

        let project = self.project.clone();
        cx.spawn_in(window, async move |workspace, cx| {
            let dirty_items = if save_intent == SaveIntent::Close && !dirty_items.is_empty() {
                let mut serialize_tasks = Vec::new();
                let mut remaining_dirty_items = Vec::new();
                if allow_hot_exit_serialization {
                    workspace.update(cx, |workspace, cx| {
                        for (pane, item) in dirty_items {
                            if let Some(task) = item
                                .to_serializable_item_handle(cx)
                                .and_then(|handle| handle.serialize(workspace, true, cx))
                            {
                                serialize_tasks.push((pane, item, task));
                            } else {
                                remaining_dirty_items.push((pane, item));
                            }
                        }
                    })?;

                    for (pane, item, task) in serialize_tasks {
                        if task.await.log_err().is_none() {
                            remaining_dirty_items.push((pane, item));
                        }
                    }
                } else {
                    remaining_dirty_items = dirty_items;
                }

                if !remaining_dirty_items.is_empty() {
                    workspace.update(cx, |_, cx| cx.emit(Event::Activate))?;
                }

                if remaining_dirty_items.len() > 1 {
                    let answer = workspace.update_in(cx, |_, window, cx| {
                        cx.emit(Event::Activate);
                        let detail = Pane::file_names_for_prompt(
                            &mut remaining_dirty_items.iter().map(|(_, handle)| handle),
                            cx,
                        );
                        window.prompt(
                            PromptLevel::Warning,
                            "Do you want to save all changes in the following files?",
                            Some(&detail),
                            &["Save all", "Discard all", "Cancel"],
                            cx,
                        )
                    })?;
                    match answer.await.log_err() {
                        Some(0) => save_intent = SaveIntent::SaveAll,
                        Some(1) => save_intent = SaveIntent::Skip,
                        Some(2) => return Ok(false),
                        _ => {}
                    }
                }

                remaining_dirty_items
            } else {
                dirty_items
            };

            for (pane, item) in dirty_items {
                let (singleton, project_entry_ids) = cx.update(|_, cx| {
                    (
                        item.buffer_kind(cx) == ItemBufferKind::Singleton,
                        item.project_entry_ids(cx),
                    )
                })?;
                if (singleton || !project_entry_ids.is_empty())
                    && !Pane::save_item(project.clone(), pane, &*item, save_intent, cx).await?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        })
    }
    pub fn save_active_item(
        &mut self,
        save_intent: SaveIntent,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let project = self.project.clone();
        let pane = self.active_pane().clone();
        let item = pane.read(cx).active_item();

        window.spawn(cx, async move |cx| {
            if let Some(item) = item {
                Pane::save_item(project, pane, item.as_ref(), save_intent, cx)
                    .await
                    .map(|_| ())
            } else {
                Ok(())
            }
        })
    }

    /// Prompts the user to save or discard each dirty item, returning
    /// `true` if they confirmed (saved/discarded everything) or `false`
    /// if they cancelled. Used before removing worktree roots during
    /// thread archival.
    pub fn prompt_to_save_or_discard_dirty_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        self.save_all_internal(SaveIntent::Close, true, window, cx)
    }

    pub fn close_inactive_items_and_panes(
        &mut self,
        action: &CloseInactiveTabsAndPanes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(task) = self.close_all_internal(
            true,
            action.save_intent.unwrap_or(SaveIntent::Close),
            window,
            cx,
        ) {
            task.detach_and_log_err(cx)
        }
    }

    pub fn close_all_items_and_panes(
        &mut self,
        action: &CloseAllItemsAndPanes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(task) = self.close_all_internal(
            false,
            action.save_intent.unwrap_or(SaveIntent::Close),
            window,
            cx,
        ) {
            task.detach_and_log_err(cx)
        }
    }

    /// Closes the active item across all panes.
    pub fn close_item_in_all_panes(
        &mut self,
        action: &CloseItemInAllPanes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_item) = self.active_pane().read(cx).active_item() else {
            return;
        };

        let save_intent = action.save_intent.unwrap_or(SaveIntent::Close);
        let close_pinned = action.close_pinned;

        if let Some(project_path) = active_item.project_path(cx) {
            self.close_items_with_project_path(
                &project_path,
                save_intent,
                close_pinned,
                window,
                cx,
            );
        } else if close_pinned || !self.active_pane().read(cx).is_active_item_pinned() {
            let item_id = active_item.item_id();
            self.active_pane().update(cx, |pane, cx| {
                pane.close_item_by_id(item_id, save_intent, window, cx)
                    .detach_and_log_err(cx);
            });
        }
    }

    /// Closes all items with the given project path across all panes.
    pub fn close_items_with_project_path(
        &mut self,
        project_path: &ProjectPath,
        save_intent: SaveIntent,
        close_pinned: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panes = self.panes().to_vec();
        for pane in panes {
            pane.update(cx, |pane, cx| {
                pane.close_items_for_project_path(
                    project_path,
                    save_intent,
                    close_pinned,
                    window,
                    cx,
                )
                .detach_and_log_err(cx);
            });
        }
    }

    fn close_all_internal(
        &mut self,
        retain_active_pane: bool,
        save_intent: SaveIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let current_pane = self.active_pane();

        let mut tasks = Vec::new();

        if retain_active_pane {
            let current_pane_close = current_pane.update(cx, |pane, cx| {
                pane.close_other_items(
                    &CloseOtherItems {
                        save_intent: None,
                        close_pinned: false,
                    },
                    None,
                    window,
                    cx,
                )
            });

            tasks.push(current_pane_close);
        }

        for pane in self.panes() {
            if retain_active_pane && pane.entity_id() == current_pane.entity_id() {
                continue;
            }

            let close_pane_items = pane.update(cx, |pane: &mut Pane, cx| {
                pane.close_all_items(
                    &CloseAllItems {
                        save_intent: Some(save_intent),
                        close_pinned: false,
                    },
                    window,
                    cx,
                )
            });

            tasks.push(close_pane_items)
        }

        if tasks.is_empty() {
            None
        } else {
            Some(cx.spawn_in(window, async move |_, _| {
                for task in tasks {
                    task.await?
                }
                Ok(())
            }))
        }
    }

    fn remove_from_session(&mut self, window: &mut Window, cx: &mut App) -> Task<()> {
        self.session_id.take();
        self.serialize_workspace_internal(window, cx)
    }

    fn flush_deferred_saves(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let deferred = std::mem::take(&mut self.deferred_save_items);
        for weak_item in deferred {
            let Some(item) = weak_item.upgrade() else {
                continue;
            };
            // Skip if focus returned to this item
            let focus_handle = item.item_focus_handle(cx);
            if focus_handle.contains_focused(window, cx) {
                continue;
            }
            Pane::autosave_item(item.as_ref(), self.project.clone(), window, cx)
                .detach_and_log_err(cx);
        }
    }
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
