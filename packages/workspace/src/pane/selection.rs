use project::{ProjectEntryId, WorktreeId};
use std::sync::Arc;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedEntry {
    pub worktree_id: WorktreeId,
    pub entry_id: ProjectEntryId,
}

#[derive(Debug)]
pub struct DraggedSelection {
    pub active_selection: SelectedEntry,
    pub marked_selections: Arc<[SelectedEntry]>,
}

impl DraggedSelection {
    pub fn items<'a>(&'a self) -> Box<dyn Iterator<Item = &'a SelectedEntry> + 'a> {
        if self.marked_selections.contains(&self.active_selection) {
            Box::new(self.marked_selections.iter())
        } else {
            Box::new(std::iter::once(&self.active_selection))
        }
    }
}
