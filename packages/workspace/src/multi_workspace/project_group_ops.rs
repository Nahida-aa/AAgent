use gpui::{App, Context, Entity, Task, WeakEntity, Window};
use util::path_list::PathList;

use super::MultiWorkspace;
use super::project_group::{
    ProjectGroup, ProjectGroupKey, ProjectGroupState, RemovalIntent, SerializedProjectGroupState,
};
use crate::Workspace;

impl MultiWorkspace {
    pub(super) fn handle_project_group_key_change(
        &mut self,
        workspace: &Entity<Workspace>,
        old_key: &ProjectGroupKey,
        cx: &mut Context<Self>,
    ) {
        if !self.is_workspace_retained(workspace) {
            return;
        }

        let new_key = workspace.read(cx).project_group_key(cx);
        if new_key.path_list().paths().is_empty() {
            return;
        }

        // The Project already emitted WorktreePathsChanged which the
        // sidebar handles for thread migration.
        self.rekey_project_group(old_key, &new_key, cx);
        self.serialize(cx);
        cx.notify();
    }
    /// Ensures a project group exists for `key`, creating one if needed.
    pub(super) fn ensure_project_group_state(&mut self, key: ProjectGroupKey) {
        if key.path_list().paths().is_empty() {
            return;
        }

        if self.project_groups.iter().any(|group| group.key == key) {
            return;
        }

        self.project_groups.insert(
            0,
            ProjectGroupState {
                key,
                expanded: true,
            },
        );
    }

    /// Transitions a project group from `old_key` to `new_key`.
    ///
    /// On collision (both keys have groups), the active workspace's
    /// Re-keys a project group from `old_key` to `new_key`, handling
    /// collisions. When two groups collide, the active workspace's
    /// group always wins. Otherwise the old key's state is preserved
    /// — it represents the group the user or system just acted on.
    /// The losing group is removed, and the winner is re-keyed in
    /// place to preserve sidebar order.
    fn rekey_project_group(
        &mut self,
        old_key: &ProjectGroupKey,
        new_key: &ProjectGroupKey,
        cx: &App,
    ) {
        if old_key == new_key {
            return;
        }

        if new_key.path_list().paths().is_empty() {
            return;
        }

        let old_key_exists = self.project_groups.iter().any(|g| g.key == *old_key);
        let new_key_exists = self.project_groups.iter().any(|g| g.key == *new_key);

        if !old_key_exists {
            self.ensure_project_group_state(new_key.clone());
            return;
        }

        if new_key_exists {
            let active_key = self.workspace().read(cx).project_group_key(cx);
            if active_key == *new_key {
                self.project_groups.retain(|g| g.key != *old_key);
            } else {
                self.project_groups.retain(|g| g.key != *new_key);
                if let Some(group) = self.project_groups.iter_mut().find(|g| g.key == *old_key) {
                    group.key = new_key.clone();
                }
            }
        } else {
            if let Some(group) = self.project_groups.iter_mut().find(|g| g.key == *old_key) {
                group.key = new_key.clone();
            }
        }

        // If another retained workspace still has the old key (e.g. a
        // linked worktree workspace), re-create the old group so it
        // remains reachable in the sidebar.
        let other_workspace_needs_old_key = self
            .held
            .iter()
            .any(|held| held.pinned && held.workspace.read(cx).project_group_key(cx) == *old_key);
        if other_workspace_needs_old_key {
            self.ensure_project_group_state(old_key.clone());
        }
    }

    pub fn project_group_key_for_workspace(
        &self,
        workspace: &Entity<Workspace>,
        cx: &App,
    ) -> ProjectGroupKey {
        workspace.read(cx).project_group_key(cx)
    }

    pub fn restore_project_groups(
        &mut self,
        groups: Vec<SerializedProjectGroupState>,
        _cx: &mut Context<Self>,
    ) {
        let mut restored: Vec<ProjectGroupState> = Vec::new();
        for SerializedProjectGroupState { key, expanded } in groups {
            if key.path_list().paths().is_empty() {
                continue;
            }
            if restored.iter().any(|group| group.key == key) {
                continue;
            }
            restored.push(ProjectGroupState { key, expanded });
        }
        for existing in std::mem::take(&mut self.project_groups) {
            if !restored.iter().any(|group| group.key == existing.key) {
                restored.push(existing);
            }
        }
        self.project_groups = restored;
    }

    pub fn project_group_keys(&self) -> Vec<ProjectGroupKey> {
        self.project_groups
            .iter()
            .map(|group| group.key.clone())
            .collect()
    }

    pub fn project_groups(&self, cx: &App) -> Vec<ProjectGroup> {
        self.project_groups
            .iter()
            .map(|group| ProjectGroup {
                key: group.key.clone(),
                workspaces: self.workspaces_for_project_group(&group.key, cx),
                expanded: group.expanded,
            })
            .collect()
    }

    pub fn last_active_workspace_for_group(
        &self,
        key: &ProjectGroupKey,
        cx: &App,
    ) -> Option<Entity<Workspace>> {
        self.held
            .iter()
            .filter(|held| held.workspace.read(cx).project_group_key(cx) == *key)
            .filter_map(|held| Some((held.activated_at?, &held.workspace)))
            .max_by_key(|(activated_at, _)| *activated_at)
            .map(|(_, workspace)| workspace.clone())
    }

    pub fn group_state_by_key(&self, key: &ProjectGroupKey) -> Option<&ProjectGroupState> {
        self.project_groups.iter().find(|group| group.key == *key)
    }

    pub fn group_state_by_key_mut(
        &mut self,
        key: &ProjectGroupKey,
    ) -> Option<&mut ProjectGroupState> {
        self.project_groups
            .iter_mut()
            .find(|group| group.key == *key)
    }

    pub fn set_all_groups_expanded(&mut self, expanded: bool) {
        for group in &mut self.project_groups {
            group.expanded = expanded;
        }
    }

    pub fn move_project_group_up(&mut self, key: &ProjectGroupKey, cx: &mut Context<Self>) -> bool {
        let Some(index) = self
            .project_groups
            .iter()
            .position(|group| group.key == *key)
        else {
            return false;
        };
        if index == 0 {
            return false;
        }
        self.project_groups.swap(index - 1, index);
        cx.emit(MultiWorkspaceEvent::ProjectGroupsChanged);
        self.serialize(cx);
        cx.notify();
        true
    }

    pub fn move_project_group_down(
        &mut self,
        key: &ProjectGroupKey,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self
            .project_groups
            .iter()
            .position(|group| group.key == *key)
        else {
            return false;
        };
        if index + 1 >= self.project_groups.len() {
            return false;
        }
        self.project_groups.swap(index, index + 1);
        cx.emit(MultiWorkspaceEvent::ProjectGroupsChanged);
        self.serialize(cx);
        cx.notify();
        true
    }

    pub fn workspaces_for_project_group(
        &self,
        key: &ProjectGroupKey,
        cx: &App,
    ) -> Vec<Entity<Workspace>> {
        self.held
            .iter()
            .filter(|held| held.pinned)
            .map(|held| held.workspace.clone())
            .filter(|workspace| workspace.read(cx).project_group_key(cx) == *key)
            .collect()
    }

    pub fn remove_project_group(
        &mut self,
        group_key: &ProjectGroupKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool>> {
        // The active workspace can remain unpinned while the sidebar is
        // closed. Pin it first: this puts it in the removal set below, and
        // stops `activate` from pinning it while switching to the
        // replacement, which would recreate the project group row this
        // function just deleted.
        let active_workspace = self.workspace().clone();
        if active_workspace.read(cx).project_group_key(cx) == *group_key
            && !self.is_workspace_retained(&active_workspace)
        {
            let index = self.hold(active_workspace, window, cx);
            self.pin(index, group_key.clone(), cx);
        }

        let workspaces = self.workspaces_for_project_group(group_key, cx);

        let task = self.remove(workspaces, RemovalIntent::CloseProject, window, cx);

        self.project_groups.retain(|group| group.key != *group_key);
        cx.emit(MultiWorkspaceEvent::ProjectGroupsChanged);

        task
    }

    /// Returns the nearest retained workspace outside the project group at
    /// `group_index`.
    ///
    /// Searches project groups by increasing distance, preferring the following
    /// group over the preceding group at equal distances. Within each group,
    /// prefers its last active workspace before falling back to any retained
    /// workspace. Workspaces in `excluded_workspaces` are ignored by both
    /// lookups.
    ///
    /// The `group_index` must identify a project group that is still present in
    /// [`Self::project_groups`].
    /// The keys of the other project groups, nearest first, preferring the
    /// following group over the preceding group at equal distances. Without an
    /// index, every group key in display order.
    fn neighbor_group_keys(&self, group_index: Option<usize>) -> Vec<ProjectGroupKey> {
        let Some(index) = group_index else {
            return self
                .project_groups
                .iter()
                .map(|group| group.key.clone())
                .collect();
        };

        (1..self.project_groups.len())
            .flat_map(|distance| [index.checked_add(distance), index.checked_sub(distance)])
            .flatten()
            .filter_map(|index| self.project_groups.get(index))
            .map(|group| group.key.clone())
            .collect()
    }

    #[cfg(test)]
    pub(super) fn nearest_retained_workspace(
        &self,
        group_index: usize,
        excluded_workspaces: &[Entity<Workspace>],
        cx: &App,
    ) -> Option<Entity<Workspace>> {
        self.neighbor_group_keys(Some(group_index))
            .into_iter()
            .find_map(|key| self.live_member_for_group(&key, excluded_workspaces, cx))
    }

    /// Goes through sqlite: serialize -> close -> open new window
    /// This avoids issues with pending tasks having the wrong window
    pub fn open_project_group_in_new_window(
        &mut self,
        key: &ProjectGroupKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let paths: Vec<PathBuf> = key.path_list().ordered_paths().cloned().collect();
        if paths.is_empty() {
            return Task::ready(Ok(()));
        }

        let app_state = self.workspace().read(cx).app_state().clone();

        let workspaces: Vec<_> = self.workspaces_for_project_group(key, cx);
        let mut serialization_tasks = Vec::new();
        for workspace in &workspaces {
            serialization_tasks.push(workspace.update(cx, |workspace, inner_cx| {
                workspace.flush_serialization(window, inner_cx)
            }));
        }

        let remove_task = self.remove_project_group(key, window, cx);

        cx.spawn(async move |_this, cx| {
            futures::future::join_all(serialization_tasks).await;

            let removed = remove_task.await?;
            if !removed {
                return Ok(());
            }

            cx.update(|cx| {
                Workspace::new_local(paths, app_state, None, None, None, OpenMode::NewWindow, cx)
            })
            .await?;

            Ok(())
        })
    }

    /// The best replacement candidate within one project group: its most
    /// recently displayed member, else its first pinned member, skipping
    /// `excluding` and disconnected projects.
    pub(super) fn live_member_for_group(
        &self,
        key: &ProjectGroupKey,
        excluding: &[Entity<Workspace>],
        cx: &App,
    ) -> Option<Entity<Workspace>> {
        let available = |workspace: &Entity<Workspace>| {
            !excluding.contains(workspace)
                && !workspace.read(cx).project().read(cx).is_disconnected(cx)
        };
        self.last_active_workspace_for_group(key, cx)
            .filter(&available)
            .or_else(|| {
                self.held
                    .iter()
                    .filter(|held| held.pinned)
                    .map(|held| held.workspace.clone())
                    .filter(|workspace| workspace.read(cx).project_group_key(cx) == *key)
                    .find(available)
            })
    }
}
