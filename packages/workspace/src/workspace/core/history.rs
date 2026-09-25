// workspace/ops/workspace_history.rs
use crate::history_manager::{HistoryManager, HistoryManagerEntry};

impl Workspace {
    pub(crate) fn update_history(&self, cx: &mut App) {
        let Some(id) = self.database_id() else {
            return;
        };
        if !self.project.read(cx).is_local() {
            return;
        }
        if let Some(manager) = HistoryManager::global(cx) {
            let paths = PathList::new(&self.root_paths(cx));
            manager.update(cx, |this, cx| {
                this.update_history(id, HistoryManagerEntry::new(id, &paths), cx);
            });
        }
    }
}
