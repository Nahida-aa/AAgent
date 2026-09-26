use super::*;

use gpui::{App, Context, Entity, Task, WeakEntity, Window};

use super::MultiWorkspace;
use super::project_group::ProjectGroupKey;
use crate::{Workspace, WorkspaceEvent};

impl MultiWorkspace {
    pub(super) fn subscribe_to_workspace(
        workspace: &Entity<Workspace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let project = workspace.read(cx).project().clone();
        cx.subscribe_in(&project, window, {
            let workspace = workspace.downgrade();
            move |this, _project, event, _window, cx| match event {
                project::Event::WorktreePathsChanged { old_worktree_paths } => {
                    if let Some(workspace) = workspace.upgrade() {
                        let host = workspace
                            .read(cx)
                            .project()
                            .read(cx)
                            .remote_connection_options(cx);
                        let old_key =
                            ProjectGroupKey::from_worktree_paths(old_worktree_paths, host);
                        this.handle_project_group_key_change(&workspace, &old_key, cx);
                    }
                }
                _ => {}
            }
        })
        .detach();

        cx.subscribe_in(workspace, window, |this, workspace, event, window, cx| {
            if let WorkspaceEvent::Activate = event {
                this.activate(workspace.clone(), None, window, cx);
            }
        })
        .detach();
    }
    pub fn workspace(&self) -> &Entity<Workspace> { &self.held[self.displayed_index()].workspace }
    pub fn workspaces(&self) -> impl Iterator<Item = &Entity<Workspace>> {
        self.held.iter().map(|held| &held.workspace)
    }
    /// Adds a workspace to this window as persistent without changing which
    /// workspace is active. Unlike `activate()`, this always inserts into the
    /// persistent list regardless of sidebar state — it's used for system-
    /// initiated additions like deserialization and worktree discovery.
    pub fn add(&mut self, workspace: Entity<Workspace>, window: &Window, cx: &mut Context<Self>) {
        if self.is_workspace_retained(&workspace) {
            return;
        }
        let key = workspace.read(cx).project_group_key(cx);
        let index = self.hold(workspace, window, cx);
        self.pin(index, key, cx);
        telemetry::event!(
            "Workspace Added",
            workspace_count = self.held.iter().filter(|held| held.pinned).count()
        );
        cx.notify();
    }

    /// Ensures the workspace is in the multiworkspace and makes it the active one.
    pub fn activate(
        &mut self,
        workspace: Entity<Workspace>,
        source_workspace: Option<WeakEntity<Workspace>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace() == &workspace {
            self.focus_active_workspace(window, cx);
            return;
        }

        let old_active_workspace = self.workspace().clone();
        let old_active_was_retained = self.active_workspace_is_retained();
        let should_retain_workspaces = self.multi_workspace_enabled(cx);

        if should_retain_workspaces && !old_active_was_retained {
            let key = old_active_workspace.read(cx).project_group_key(cx);
            let index = self.hold(old_active_workspace.clone(), window, cx);
            self.pin(index, key, cx);
        }

        let displayed = self.hold(workspace.clone(), window, cx);
        if should_retain_workspaces {
            let key = workspace.read(cx).project_group_key(cx);
            self.pin(displayed, key, cx);
        }

        // Publish the new active workspace before anyone reads the shared cell
        // to decide who owns the window chrome.
        self.active_workspace_id.set(workspace.entity_id());

        let stamp = self
            .held
            .iter()
            .filter_map(|held| held.activated_at)
            .max()
            .map_or(0, |max| max + 1);
        self.held[displayed].activated_at = Some(stamp);

        if !should_retain_workspaces && !old_active_was_retained {
            self.detach_workspace(&old_active_workspace, cx);
        }

        // The platform window is shared across all workspaces in this window.
        // The previously-active workspace left the title and edited indicator
        // reflecting its own state, so re-apply them from the newly-active
        // workspace (which is now the chrome owner per `owns_window_chrome`).
        workspace.update(cx, |workspace, cx| {
            workspace.refresh_window_state(window, cx);
        });

        cx.emit(MultiWorkspaceEvent::ActiveWorkspaceChanged { source_workspace });
        self.serialize(cx);
        self.focus_active_workspace(window, cx);
        cx.notify();
    }
    pub(crate) fn activate_provisional_workspace(
        &mut self,
        workspace: Entity<Workspace>,
        provisional_key: ProjectGroupKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.hold(workspace.clone(), window, cx);
        self.pin(index, provisional_key, cx);
        self.activate(workspace, None, window, cx);
    }
    pub fn focus_active_workspace(&self, window: &mut Window, cx: &mut App) {
        let focus_handle = self.workspace().read(cx).fallback_focus_handle(window, cx);
        window.focus(&focus_handle, cx);
    }
}
