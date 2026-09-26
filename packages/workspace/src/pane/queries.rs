use super::*;

use gpui::{App, Context, FocusOutEvent, Window};
use project::Project;
use settings::Settings;
use theme_settings::ThemeSettings;

use super::Pane;
use super::history::{NavigationMode, TagNavigationMode};
use crate::item::{ItemSettings, PreviewTabsSettings};
use crate::workspace_settings::{FocusFollowsMouse, TabBarSettings, WorkspaceSettings};

impl Pane {
    pub fn items_len(&self) -> usize { self.items.len() }
    pub fn items(&self) -> impl DoubleEndedIterator<Item = &Box<dyn ItemHandle>> {
        self.items.iter()
    }

    pub fn items_of_type<T: Render>(&self) -> impl '_ + Iterator<Item = Entity<T>> {
        self.items
            .iter()
            .filter_map(|item| item.to_any_view().downcast().ok())
    }

    pub fn active_item(&self) -> Option<Box<dyn ItemHandle>> {
        self.items.get(self.active_item_index).cloned()
    }

    pub(super) fn active_item_id(&self) -> EntityId { self.items[self.active_item_index].item_id() }

    pub fn active_item_index(&self) -> usize { self.active_item_index }
    pub fn is_active_item_pinned(&self) -> bool { self.is_tab_pinned(self.active_item_index) }
    pub fn activation_history(&self) -> &[ActivationHistoryEntry] { &self.activation_history }
    pub fn pixel_position_of_cursor(&self, cx: &App) -> Option<Point<Pixels>> {
        self.items
            .get(self.active_item_index)?
            .pixel_position_of_cursor(cx)
    }
    pub fn item_for_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &App,
    ) -> Option<Box<dyn ItemHandle>> {
        self.items.iter().find_map(|item| {
            if item.buffer_kind(cx) == ItemBufferKind::Singleton
                && (item.project_entry_ids(cx).as_slice() == [entry_id])
            {
                Some(item.boxed_clone())
            } else {
                None
            }
        })
    }
    pub fn item_for_path(
        &self,
        project_path: ProjectPath,
        cx: &App,
    ) -> Option<Box<dyn ItemHandle>> {
        self.items.iter().find_map(move |item| {
            if item.buffer_kind(cx) == ItemBufferKind::Singleton
                && (item.project_path(cx).as_slice() == [project_path.clone()])
            {
                Some(item.boxed_clone())
            } else {
                None
            }
        })
    }
    pub fn index_for_item(&self, item: &dyn ItemHandle) -> Option<usize> {
        self.index_for_item_id(item.item_id())
    }
    pub(super) fn index_for_item_id(&self, item_id: EntityId) -> Option<usize> {
        self.items.iter().position(|i| i.item_id() == item_id)
    }

    pub fn item_for_index(&self, ix: usize) -> Option<&dyn ItemHandle> {
        self.items.get(ix).map(|i| i.as_ref())
    }
    pub fn toolbar(&self) -> &Entity<Toolbar> { &self.toolbar }
    pub fn icon_color(selected: bool) -> Color {
        if selected {
            Color::Default
        } else {
            Color::Muted
        }
    }
    pub(super) fn entry_abs_path(&self, entry: ProjectEntryId, cx: &App) -> Option<PathBuf> {
        let worktree = self
            .workspace
            .upgrade()?
            .read(cx)
            .project()
            .read(cx)
            .worktree_for_entry(entry, cx)?
            .read(cx);
        let entry = worktree.entry_for_id(entry)?;
        Some(match &entry.canonical_path {
            Some(canonical_path) => canonical_path.to_path_buf(),
            None => worktree.absolutize(&entry.path),
        })
    }
    pub(super) fn pinned_item_ids(&self) -> Vec<EntityId> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if self.is_tab_pinned(index) {
                    return Some(item.item_id());
                }

                None
            })
            .collect()
    }
    pub(super) fn clean_item_ids(&self, cx: &mut Context<Pane>) -> Vec<EntityId> {
        self.items()
            .filter_map(|item| {
                if !item.is_dirty(cx) {
                    return Some(item.item_id());
                }

                None
            })
            .collect()
    }
    pub(super) fn to_the_side_item_ids(&self, item_id: EntityId, side: Side) -> Vec<EntityId> {
        match side {
            Side::Left => self
                .items()
                .take_while(|item| item.item_id() != item_id)
                .map(|item| item.item_id())
                .collect(),
            Side::Right => self
                .items()
                .rev()
                .take_while(|item| item.item_id() != item_id)
                .map(|item| item.item_id())
                .collect(),
        }
    }
    pub(super) fn multibuffer_item_ids(&self, cx: &mut Context<Pane>) -> Vec<EntityId> {
        self.items()
            .filter(|item| item.buffer_kind(cx) == ItemBufferKind::Multibuffer)
            .map(|item| item.item_id())
            .collect()
    }
}
