use anyhow::{Context as _, Result, anyhow};
use futures::future::join_all;
use gpui::{App, AppContext as _, Context, Entity, Task, TaskExt as _};
use util::rel_path::RelPath;

use super::Project;
use crate::types::*;
use crate::{Event, ProjectPath};
use crate::worktree_store::{WorktreeStoreEvent, WorktreePaths};

impl Project {
    pub fn worktrees<'a>(&self, cx: &'a App) -> impl 'a + DoubleEndedIterator<Item = Entity<worktree::Worktree>> { /* 原样 */ }
    pub fn visible_worktrees<'a>(&'a self, cx: &'a App) -> impl 'a + DoubleEndedIterator<Item = Entity<worktree::Worktree>> { /* 原样 */ }
    pub(crate) fn default_visible_worktree_paths(worktree_store: &WorktreeStore, cx: &App) -> Vec<std::path::PathBuf> { /* 原样 */ }
    pub fn default_path_list(&self, cx: &App) -> util::path_list::PathList { /* 原样 */ }
    pub fn worktree_for_root_name(&self, root_name: &str, cx: &App) -> Option<Entity<worktree::Worktree>> { /* 原样 */ }
    pub fn worktree_root_names<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = &'a str> { /* 原样 */ }
    pub fn worktree_for_id(&self, id: worktree::WorktreeId, cx: &App) -> Option<Entity<worktree::Worktree>> { /* 原样 */ }
    pub fn worktree_for_entry(&self, entry_id: worktree::ProjectEntryId, cx: &App) -> Option<Entity<worktree::Worktree>> { /* 原样 */ }
    pub fn worktree_id_for_entry(&self, entry_id: worktree::ProjectEntryId, cx: &App) -> Option<worktree::WorktreeId> { /* 原样 */ }
    pub fn entry_is_worktree_root(&self, entry_id: worktree::ProjectEntryId, cx: &App) -> bool { /* 原样 */ }
    pub fn find_or_create_worktree(&mut self, abs_path: impl AsRef<std::path::Path>, visible: bool, cx: &mut Context<Self>) -> Task<Result<(Entity<worktree::Worktree>, Arc<RelPath>)>> { /* 原样 */ }
    pub fn find_worktree(&self, abs_path: &std::path::Path, cx: &App) -> Option<(Entity<worktree::Worktree>, Arc<RelPath>)> { /* 原样 */ }
    pub fn create_worktree(&mut self, abs_path: impl AsRef<std::path::Path>, visible: bool, cx: &mut Context<Self>) -> Task<Result<Entity<worktree::Worktree>>> { /* 原样 */ }
    pub fn wait_for_worktree_release(&mut self, worktree_id: worktree::WorktreeId, cx: &mut Context<Self>) -> Task<Result<()>> { /* 原样 */ }
    pub fn remove_worktree(&mut self, id_to_remove: worktree::WorktreeId, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn remove_worktree_for_main_worktree_path(&mut self, path: impl AsRef<std::path::Path>, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn move_worktree(&mut self, source: worktree::WorktreeId, destination: worktree::WorktreeId, cx: &mut Context<Self>) -> Result<()> { /* 原样 */ }
    pub(crate) fn add_worktree(&mut self, worktree: &Entity<worktree::Worktree>, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn worktree_metadata_protos(&self, cx: &App) -> Vec<proto::WorktreeMetadata> { /* 原样 */ }
    pub fn worktree_paths(&self, cx: &App) -> WorktreePaths { /* 原样 */ }
    pub fn path_style(&self, cx: &App) -> util::paths::PathStyle { /* 原样 */ }
    pub fn contains_local_settings_file(&self, worktree_id: worktree::WorktreeId, rel_path: &RelPath, cx: &App) -> bool { /* 原样 */ }
    pub(crate) fn emit_group_key_changed_if_needed(&mut self, cx: &mut Context<Self>) { /* 原样 */ }
    pub(crate) fn on_worktree_store_event(&mut self, _: Entity<WorktreeStore>, event: &WorktreeStoreEvent, cx: &mut Context<Self>) { /* 原样 */ }
    fn on_worktree_added(&mut self, worktree: &Entity<worktree::Worktree>, _: &mut Context<Self>) { /* 原样 */ }
    fn on_worktree_released(&mut self, id_to_remove: worktree::WorktreeId, cx: &mut Context<Self>) { /* 原样 */ }
    fn set_worktrees_from_proto(&mut self, worktrees: Vec<proto::WorktreeMetadata>, cx: &mut Context<Project>) -> Result<()> { /* 原样 */ }
    pub fn create_entry(&mut self, project_path: impl Into<ProjectPath>, is_directory: bool, cx: &mut Context<Self>) -> Task<Result<worktree::CreatedEntry>> { /* 原样 */ }
    pub fn copy_entry(&mut self, entry_id: worktree::ProjectEntryId, new_project_path: ProjectPath, cx: &mut Context<Self>) -> Task<Result<Option<worktree::Entry>>> { /* 原样 */ }
    pub fn rename_entry(&mut self, entry_id: worktree::ProjectEntryId, new_path: ProjectPath, cx: &mut Context<Self>) -> Task<Result<worktree::CreatedEntry>> { /* 原样 */ }
    pub fn trash_file(&mut self, path: ProjectPath, cx: &mut Context<Self>) -> Option<Task<Result<fs::TrashId>>> { /* 原样 */ }
    pub fn delete_file(&mut self, path: ProjectPath, cx: &mut Context<Self>) -> Option<Task<Result<()>>> { /* 原样 */ }
    pub fn trash_entry(&mut self, entry_id: worktree::ProjectEntryId, cx: &mut Context<Self>) -> Option<Task<Result<fs::TrashId>>> { /* 原样 */ }
    pub fn delete_entry(&mut self, entry_id: worktree::ProjectEntryId, cx: &mut Context<Self>) -> Option<Task<Result<()>>> { /* 原样 */ }
    pub fn restore_entry(&self, worktree_id: worktree::WorktreeId, trash_id: fs::TrashId, cx: &mut Context<'_, Self>) -> Task<Result<ProjectPath>> { /* 原样 */ }
    pub fn expand_entry(&mut self, worktree_id: worktree::WorktreeId, entry_id: worktree::ProjectEntryId, cx: &mut Context<Self>) -> Option<Task<Result<()>>> { /* 原样 */ }
    pub fn expand_all_for_entry(&mut self, worktree_id: worktree::WorktreeId, entry_id: worktree::ProjectEntryId, cx: &mut Context<Self>) -> Option<Task<Result<()>>> { /* 原样 */ }
    pub fn entry_for_path<'a>(&'a self, path: &ProjectPath, cx: &'a App) -> Option<&'a worktree::Entry> { /* 原样 */ }
    pub fn path_for_entry(&self, entry_id: worktree::ProjectEntryId, cx: &App) -> Option<ProjectPath> { /* 原样 */ }
    pub fn absolute_path(&self, project_path: &ProjectPath, cx: &App) -> Option<std::path::PathBuf> { /* 原样 */ }
}
