use super::*;
use super::Workspace;
use crate::{dock::Dock, workspace::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};



impl Workspace {
    pub fn show_worktree_trust_security_modal(
        &mut self,
        toggle: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(security_modal) = self.active_modal::<SecurityModal>(cx) {
            if toggle {
                security_modal.update(cx, |security_modal, cx| {
                    security_modal.dismiss(cx);
                })
            } else {
                security_modal.update(cx, |security_modal, cx| {
                    security_modal.refresh_restricted_paths(cx);
                });
            }
        } else {
            let has_restricted_worktrees = TrustedWorktrees::has_restricted_worktrees(
                &self.project().read(cx).worktree_store(),
                cx,
            );
            if has_restricted_worktrees {
                let project = self.project().read(cx);
                let remote_host = project
                    .remote_connection_options(cx)
                    .map(RemoteHostLocation::from);
                let worktree_store = project.worktree_store().downgrade();
                self.toggle_modal(window, cx, |window, cx| {
                    SecurityModal::new(worktree_store, remote_host, window, cx)
                });
            }
        }
    }
}
