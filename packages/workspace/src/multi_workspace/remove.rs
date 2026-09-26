use super::*;

use anyhow::Result;
use gpui::{Context, Entity, Task, Window};

use super::MultiWorkspace;
use super::project_group::RemovalIntent;
use crate::{CloseIntent, Workspace};

impl MultiWorkspace {
    /// Removes one or more workspaces from this multi-workspace.
    ///
    /// Every workspace is first asked for consent (save prompts); no state
    /// changes until all consent. The rows are then deleted and, when the
    /// displayed workspace was among them, a replacement is chosen from what
    /// remains: another workspace in the same project, then a workspace in the
    /// nearest neighboring project, then an empty workspace. When the intent is
    /// `KeepProject` and the project has no other workspace, its root worktrees
    /// are reopened afterwards; the same applies to the adjacent local project
    /// when nothing at all remains.
    ///
    /// Returns `true` if any workspaces were actually removed.
    pub fn remove(
        &mut self,
        workspaces: impl IntoIterator<Item = Entity<Workspace>>,
        intent: RemovalIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        let workspaces: Vec<_> = workspaces.into_iter().collect();

        if workspaces.is_empty() {
            return Task::ready(Ok(false));
        }

        let original_active = self.workspace().clone();
        let group_key = original_active.read(cx).project_group_key(cx);

        // Record the neighborhood of the project as the user sees it now:
        // callers like `remove_project_group` delete the project row itself
        // before the removal task runs.
        let group_index = self
            .project_groups
            .iter()
            .position(|group| group.key == group_key);
        let neighbor_keys = self.neighbor_group_keys(group_index);
        let adjacent_key = group_index.and_then(|index| {
            self.project_groups
                .get(index + 1)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|previous| self.project_groups.get(previous))
                })
                .map(|group| group.key.clone())
        });

        cx.spawn_in(window, async move |this, cx| {
            // Consent phase: run the standard close lifecycle for every
            // workspace being removed. Prompts only; no state changes.
            for workspace in &workspaces {
                let should_continue = workspace
                    .update_in(cx, |workspace, window, cx| {
                        workspace.prepare_to_close(CloseIntent::ReplaceWindow, window, cx)
                    })?
                    .await?;

                if !should_continue {
                    return Ok(false);
                }
            }

            // Edit phase: one synchronous update. Delete the rows, then pick
            // the replacement from the rows that actually remain.
            let (removed_any, reopen_key) = this.update_in(cx, |this, window, cx| {
                let mut removed_any = false;
                let displayed_workspace = this.workspace().clone();

                for workspace in &workspaces {
                    if *workspace == displayed_workspace {
                        continue;
                    }
                    if this.held_index(workspace).is_some() {
                        this.detach_workspace(workspace, cx);
                        removed_any = true;
                    }
                }

                let mut reopen_key = None;
                if workspaces.contains(&displayed_workspace) {
                    let doomed = std::slice::from_ref(&displayed_workspace);

                    let same_group = this
                        .held
                        .iter()
                        .filter(|held| held.pinned && held.workspace != displayed_workspace)
                        .map(|held| held.workspace.clone())
                        .find(|workspace| workspace.read(cx).project_group_key(cx) == group_key);
                    if intent == RemovalIntent::KeepProject
                        && same_group.is_none()
                        && group_key.host().is_none()
                        && !group_key.path_list().is_empty()
                    {
                        reopen_key = Some(group_key.clone());
                    }

                    let replacement = same_group
                        .or_else(|| {
                            neighbor_keys
                                .iter()
                                .find_map(|key| this.live_member_for_group(key, doomed, cx))
                        })
                        .unwrap_or_else(|| {
                            if reopen_key.is_none() {
                                reopen_key = adjacent_key.clone().filter(|key| {
                                    key.host().is_none() && !key.path_list().is_empty()
                                });
                            }
                            let app_state = displayed_workspace.read(cx).app_state().clone();
                            let project = Project::local(
                                app_state.client.clone(),
                                app_state.node_runtime.clone(),
                                app_state.user_store.clone(),
                                app_state.languages.clone(),
                                app_state.fs.clone(),
                                None,
                                project::LocalProjectFlags::default(),
                                cx,
                            );
                            cx.new(|cx| Workspace::new(None, project, app_state, window, cx))
                        });

                    this.activate(replacement, None, window, cx);
                    this.detach_workspace(&displayed_workspace, cx);
                    removed_any = true;
                } else if *this.workspace() != original_active
                    && !workspaces.contains(&original_active)
                {
                    // Prompting switched the display away from where the user
                    // was; go back.
                    this.activate(original_active.clone(), None, window, cx);
                }

                if removed_any {
                    this.serialize(cx);
                    cx.notify();
                }

                (removed_any, reopen_key)
            })?;

            if let Some(key) = reopen_key {
                this.update_in(cx, |this, window, cx| {
                    this.find_or_create_local_workspace(
                        key.path_list().clone(),
                        Some(key),
                        None,
                        OpenMode::Activate,
                        None,
                        window,
                        cx,
                    )
                })?
                .await?;
            }

            Ok(removed_any)
        })
    }
}
