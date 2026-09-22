use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::App;
use util::rel_path::RelPath;
use util::paths::{PathStyle, SanitizedPath};

use super::Project;
use crate::path::ResolvedPath;
use crate::ProjectPath;

impl Project {
    pub fn find_project_path(&self, path: impl AsRef<Path>, cx: &App) -> Option<ProjectPath> { /* 原样 */ }
    pub fn short_full_path_for_project_path(&self, project_path: &ProjectPath, cx: &App) -> Option<String> { /* 原样 */ }
    pub fn project_path_for_absolute_path(&self, abs_path: &Path, cx: &App) -> Option<ProjectPath> { /* 原样 */ }
    pub fn get_workspace_root(&self, project_path: &ProjectPath, cx: &App) -> Option<PathBuf> { /* 原样 */ }
    pub fn visibility_for_paths(&self, paths: &[PathBuf], exclude_sub_dirs: bool, cx: &App) -> Option<bool> { /* 原样 */ }
    pub fn visibility_for_path(&self, path: &Path, exclude_sub_dirs: bool, cx: &App) -> Option<bool> { /* 原样 */ }
    pub fn visibility_for_subpaths(&self, paths: &[PathBuf], cx: &App) -> Option<bool> { /* 原样 */ }
    fn visibility_for_subpath(&self, path: &Path, cx: &App) -> Option<bool> { /* 原样 */ }
    pub fn resolve_path_in_buffer(&self, path: &str, buffer: &gpui::Entity<language::Buffer>, cx: &mut gpui::Context<Self>) -> gpui::Task<Option<ResolvedPath>> { /* 原样 */ }
    pub fn resolve_abs_file_path(&self, path: &str, cx: &mut gpui::Context<Self>) -> gpui::Task<Option<ResolvedPath>> { /* 原样 */ }
    pub fn resolve_abs_path(&self, path: &str, cx: &App) -> gpui::Task<Option<ResolvedPath>> { /* 原样 */ }
    fn resolve_path_in_worktrees(&self, path: &str, buffer: &gpui::Entity<language::Buffer>, cx: &mut gpui::Context<Self>) -> gpui::Task<Option<ResolvedPath>> { /* 原样 */ }
    fn resolve_path_in_worktree(worktree: &gpui::Entity<worktree::Worktree>, path: &RelPath, cx: &mut gpui::AsyncApp) -> Option<ResolvedPath> { /* 原样 */ }
    pub fn try_windows_path_to_wsl(&self, abs_path: &Path, cx: &App) -> impl std::future::Future<Output = anyhow::Result<PathBuf>> + use<> { /* 原样 */ }
    pub fn list_directory(&self, query: String, cx: &mut gpui::Context<Self>) -> gpui::Task<anyhow::Result<Vec<crate::directory::DirectoryItem>>> { /* 原样 */ }
}
