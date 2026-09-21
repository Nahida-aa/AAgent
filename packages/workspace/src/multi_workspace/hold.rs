use gpui::{App, Context, Entity, EntityId, WeakEntity, Window};

use super::MultiWorkspace;
use super::project_group::ProjectGroupKey;
use crate::Workspace;

pub(super) struct HeldWorkspace {
    pub(super) workspace: Entity<Workspace>,
    pub(super) pinned: bool,
    pub(super) activated_at: Option<u64>,
}

impl MultiWorkspace {
    fn held_index(&self, workspace: &Entity<Workspace>) -> Option<usize> {
        self.held
            .iter()
            .position(|held| held.workspace == *workspace)
    }

    pub fn is_workspace_retained(&self, workspace: &Entity<Workspace>) -> bool {
        self.held
            .iter()
            .any(|held| held.pinned && held.workspace == *workspace)
    }

    pub fn active_workspace_is_retained(&self) -> bool { self.held[self.displayed_index()].pinned }

    /// The displayed workspace is the most recently activated row.
    pub(super) fn displayed_index(&self) -> usize {
        self.held
            .iter()
            .enumerate()
            .max_by_key(|(_, held)| held.activated_at)
            .expect("a window always holds at least one workspace")
            .0
    }
    /// Ensures `workspace` has a row in `held`, registering it on first
    /// insert, and returns the row's index.
    pub(super) fn hold(
        &mut self,
        workspace: Entity<Workspace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> usize {
        if let Some(index) = self.held_index(&workspace) {
            return index;
        }
        self.register_workspace(&workspace, window, cx);
        self.held.push(HeldWorkspace {
            workspace,
            pinned: false,
            activated_at: None,
        });
        self.held.len() - 1
    }

    /// Pins the row so the workspace survives navigating away, recording
    /// `group` as the project group it was pinned under. No-op if already
    /// pinned.
    pub(super) fn pin(&mut self, index: usize, group: ProjectGroupKey, cx: &mut Context<Self>) {
        if self.held[index].pinned {
            return;
        }
        self.held[index].pinned = true;
        self.ensure_project_group_state(group);
        cx.emit(MultiWorkspaceEvent::WorkspaceAdded(
            self.held[index].workspace.clone(),
        ));
    }
    pub(super) fn register_workspace(
        &mut self,
        workspace: &Entity<Workspace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        Self::subscribe_to_workspace(workspace, window, cx);
        let weak_self = cx.weak_entity();
        let active_workspace_id = self.active_workspace_id.clone();
        workspace.update(cx, |workspace, cx| {
            workspace.set_multi_workspace(weak_self, active_workspace_id, cx);
        });

        let entity = cx.entity();
        cx.defer({
            let workspace = workspace.clone();
            move |cx| {
                entity.update(cx, |this, cx| {
                    this.sync_sidebar_to_workspace(&workspace, cx);
                })
            }
        });
    }

    /// Promotes the currently active workspace to persistent if it is
    /// transient, so it is retained across workspace switches even when
    /// the sidebar is closed. No-op if the workspace is already persistent.
    pub fn retain_active_workspace(&mut self, cx: &mut Context<Self>) {
        let index = self.displayed_index();
        if self.held[index].pinned {
            return;
        }
        let key = self.held[index].workspace.read(cx).project_group_key(cx);
        self.pin(index, key, cx);
        self.serialize(cx);
        cx.notify();
    }

    /// Collapses to a single workspace, discarding all groups.
    /// Used when multi-workspace is disabled by settings.
    fn collapse_to_single_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_open {
            self.close_sidebar(window, cx);
        }

        let displayed_workspace = self.workspace().clone();
        for workspace in self.workspaces().cloned().collect::<Vec<_>>() {
            if workspace != displayed_workspace {
                self.detach_workspace(&workspace, cx);
            }
        }

        for held in &mut self.held {
            held.pinned = false;
        }
        self.project_groups.clear();
        cx.notify();
    }

    /// Detaches a workspace: clears session state, DB binding, cached
    /// group key, and emits `WorkspaceRemoved`. The DB row is preserved
    /// so the workspace still appears in the recent-projects list.
    pub(super) fn detach_workspace(
        &mut self,
        workspace: &Entity<Workspace>,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.held_index(workspace) {
            assert_ne!(
                index,
                self.displayed_index(),
                "the displayed workspace must be re-pointed before it is detached"
            );
            self.held.remove(index);
        }
        cx.emit(MultiWorkspaceEvent::WorkspaceRemoved(workspace.entity_id()));
        workspace.update(cx, |workspace, _cx| {
            workspace.session_id.take();
            workspace._schedule_serialize_workspace.take();
            workspace._serialize_workspace_task.take();
        });

        if let Some(workspace_id) = workspace.read(cx).database_id() {
            let db = crate::persistence::WorkspaceDb::global(cx);
            self.pending_removal_tasks.retain(|task| !task.is_ready());
            self.pending_removal_tasks
                .push(cx.background_spawn(async move {
                    db.set_session_binding(workspace_id, None, None)
                        .await
                        .log_err();
                }));
        }
    }
}
