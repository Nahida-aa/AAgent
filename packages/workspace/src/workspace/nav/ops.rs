use super::*;
use super::Workspace;
use crate::{dock::Dock, workspace::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {

    // 写入与前进/后退
    // pub fn clear_navigation_history
    pub fn clear_navigation_history(&mut self, window: &mut Window, cx: &mut Context<Workspace>) {
        for pane in &self.panes {
            pane.update(cx, |pane, cx| pane.nav_history_mut().clear(cx));
        }
        self.persisted_recent_navigation_history.clear();
        self.last_active_project_path = None;
        self.serialize_workspace(window, cx);
    }
    // fn rename_persisted_navigation_history_paths     // private
    pub(crate) fn rename_persisted_navigation_history_paths(
        &mut self,
        old_path: &Path,
        new_path: &Path,
    ) -> bool {
        let mut changed = false;
        for path in &mut self.persisted_recent_navigation_history {
            let Ok(suffix) = path.strip_prefix(old_path) else {
                continue;
            };
            let renamed_path = new_path.join(suffix);
            if *path != renamed_path {
                *path = renamed_path;
                changed = true;
            }
        }

        if changed {
            let mut seen_paths = HashSet::default();
            self.persisted_recent_navigation_history
                .retain(|path| seen_paths.insert(path.clone()));
        }
        changed
    }
    // fn remember_navigation_history_path              // private
    pub(crate) fn remember_navigation_history_path(&mut self, project_path: &ProjectPath, cx: &App) -> bool {
        if self.restoring_workspace {
            return false;
        }
        let Some(absolute_path) = self.project.read(cx).absolute_path(project_path, cx) else {
            return false;
        };

        self.last_active_project_path = Some(project_path.clone());

        if self.persisted_recent_navigation_history.first() == Some(&absolute_path) {
            return false;
        }

        self.persisted_recent_navigation_history
            .retain(|path| path != &absolute_path);
        self.persisted_recent_navigation_history
            .insert(0, absolute_path);
        self.persisted_recent_navigation_history
            .truncate(MAX_RECENT_SELECTIONS);
        true
    }
    // fn navigate_history                              // private
    pub(crate) fn navigate_history(
        &mut self,
        pane: WeakEntity<Pane>,
        mode: NavigationMode,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        self.navigate_history_impl(
            pane,
            mode,
            window,
            &mut |history, cx| history.pop(mode, cx),
            cx,
        )
    }
    // fn navigate_tag_history                          // private
    pub(crate) fn navigate_tag_history(
        &mut self,
        pane: WeakEntity<Pane>,
        mode: TagNavigationMode,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        self.navigate_history_impl(
            pane,
            NavigationMode::Normal,
            window,
            &mut |history, _cx| history.pop_tag(mode),
            cx,
        )
    }
    // fn navigate_history_impl                         // private
    pub(crate) fn navigate_history_impl(
        &mut self,
        pane: WeakEntity<Pane>,
        mode: NavigationMode,
        window: &mut Window,
        cb: &mut dyn FnMut(&mut NavHistory, &mut App) -> Option<NavigationEntry>,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        let to_load = if let Some(pane) = pane.upgrade() {
            pane.update(cx, |pane, cx| {
                window.focus(&pane.focus_handle(cx), cx);
                loop {
                    // Retrieve the weak item handle from the history.
                    let entry = cb(pane.nav_history_mut(), cx)?;

                    // If the item is still present in this pane, then activate it.
                    if let Some(index) = entry
                        .item
                        .upgrade()
                        .and_then(|v| pane.index_for_item(v.as_ref()))
                    {
                        let prev_active_item_index = pane.active_item_index();
                        pane.nav_history_mut().set_mode(mode);
                        pane.activate_item(index, true, true, window, cx);
                        pane.nav_history_mut().set_mode(NavigationMode::Normal);

                        let mut navigated = prev_active_item_index != pane.active_item_index();
                        if let Some(data) = entry.data {
                            navigated |= pane.active_item()?.navigate(data, window, cx);
                        }

                        if navigated {
                            break None;
                        }
                    } else {
                        // If the item is no longer present in this pane, then retrieve its
                        // path info in order to reopen it.
                        if let Some((project_path, abs_path)) =
                            pane.nav_history().path_for_item(entry.item.id())
                        {
                            break Some((project_path, abs_path, entry));
                        }
                    }
                }
            })
        } else {
            None
        };

        if let Some((project_path, abs_path, entry)) = to_load {
            // If the item was no longer present, then load it again from its previous path, first try the local path
            let open_by_project_path = self.load_path(project_path.clone(), window, cx);

            cx.spawn_in(window, async move  |workspace, cx| {
                let open_by_project_path = open_by_project_path.await;
                let mut navigated = false;
                match open_by_project_path
                    .with_context(|| format!("Navigating to {project_path:?}"))
                {
                    Ok((project_entry_id, build_item)) => {
                        let prev_active_item_id = pane.update(cx, |pane, _| {
                            pane.nav_history_mut().set_mode(mode);
                            pane.active_item().map(|p| p.item_id())
                        })?;

                        pane.update_in(cx, |pane, window, cx| {
                            let item = pane.open_item(
                                project_entry_id,
                                project_path,
                                true,
                                entry.is_preview,
                                true,
                                None,
                                window, cx,
                                build_item,
                            );
                            navigated |= Some(item.item_id()) != prev_active_item_id;
                            pane.nav_history_mut().set_mode(NavigationMode::Normal);
                            if let Some(data) = entry.data {
                                navigated |= item.navigate(data, window, cx);
                            }
                        })?;
                    }
                    Err(open_by_project_path_e) => {
                        // Fall back to opening by abs path, in case an external file was opened and closed,
                        // and its worktree is now dropped
                        if let Some(abs_path) = abs_path {
                            let prev_active_item_id = pane.update(cx, |pane, _| {
                                pane.nav_history_mut().set_mode(mode);
                                pane.active_item().map(|p| p.item_id())
                            })?;
                            let open_by_abs_path = workspace.update_in(cx, |workspace, window, cx| {
                                workspace.open_abs_path(abs_path.clone(), OpenOptions { visible: Some(OpenVisible::None), ..Default::default() }, window, cx)
                            })?;
                            match open_by_abs_path
                                .await
                                .with_context(|| format!("Navigating to {abs_path:?}"))
                            {
                                Ok(item) => {
                                    pane.update_in(cx, |pane, window, cx| {
                                        navigated |= Some(item.item_id()) != prev_active_item_id;
                                        pane.nav_history_mut().set_mode(NavigationMode::Normal);
                                        if let Some(data) = entry.data {
                                            navigated |= item.navigate(data, window, cx);
                                        }
                                    })?;
                                }
                                Err(open_by_abs_path_e) => {
                                    log::error!("Failed to navigate history: {open_by_project_path_e:#} and {open_by_abs_path_e:#}");
                                }
                            }
                        }
                    }
                }

                if !navigated {
                    workspace
                        .update_in(cx, |workspace, window, cx| {
                            Self::navigate_history(workspace, pane, mode, window, cx)
                        })?
                        .await?;
                }

                Ok(())
            })
        } else {
            Task::ready(Ok(()))
        }
    }
    // pub fn go_back
    pub fn go_back(
        &mut self,
        pane: WeakEntity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        self.navigate_history(pane, NavigationMode::GoingBack, window, cx)
    }
    // pub fn go_forward
    pub fn go_forward(
        &mut self,
        pane: WeakEntity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        self.navigate_history(pane, NavigationMode::GoingForward, window, cx)
    }
    // pub fn reopen_closed_item
    pub fn reopen_closed_item(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<()>> {
        self.navigate_history(
            self.active_pane().downgrade(),
            NavigationMode::ReopeningClosedItem,
            window,
            cx,
        )
    }
    // pub(crate) fn active_item_path_changed   // 更新导航历史 + 标题
    pub(crate) fn active_item_path_changed(
        &mut self,
        focus_changed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.emit(Event::ActiveItemChanged);
        let active_entry = self.active_project_path(cx);
        let active_project_path_changed =
            self.last_active_project_path.as_ref() != active_entry.as_ref();
        self.project.update(cx, |project, cx| {
            project.set_active_path(active_entry.clone(), cx)
        });

        if focus_changed && let Some(project_path) = &active_entry {
            let git_store_entity = self.project.read(cx).git_store().clone();
            git_store_entity.update(cx, |git_store, cx| {
                git_store.set_active_repo_for_path(project_path, cx);
            });
        }

        if active_project_path_changed {
            match active_entry.as_ref() {
                None => self.last_active_project_path = None,
                Some(path) if self.remember_navigation_history_path(path, cx) => {
                    self.serialize_workspace(window, cx);
                }
                Some(_) => {}
            }
        }

        self.update_window_title(window, cx);
    }

}
