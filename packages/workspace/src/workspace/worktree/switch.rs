use super::Workspace;
use crate::{dock::Dock, workspace::event::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};


impl Workspace {

    /// Captures the current workspace state for restoring after a worktree switch.
    /// This includes dock layout, open file paths, and the active file path.
    pub fn capture_state_for_worktree_switch(
        &self,
        window: &Window,
        fallback_focused_dock: Option<DockPosition>,
        cx: &App,
    ) -> PreviousWorkspaceState {
        let dock_structure = self.capture_dock_state(window, cx);
        let open_file_paths = self.open_item_abs_paths(cx);
        let active_file_path = self
            .active_item(cx)
            .and_then(|item| item.project_path(cx))
            .and_then(|pp| self.project().read(cx).absolute_path(&pp, cx));

        let focused_dock = self
            .focused_dock_position(window, cx)
            .or(fallback_focused_dock);

        PreviousWorkspaceState {
            dock_structure,
            open_file_paths,
            active_file_path,
            focused_dock,
        }
    }



}
