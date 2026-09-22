#![cfg(feature = "test-support")]

use std::path::Path;

use gpui::{App, AsyncApp, Context, Entity, TestAppContext};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use util::paths::PathStyle;

use super::Project;
use crate::types::*;
use crate::ProjectEnvironmentEvent;

impl Project {
    pub fn client_subscriptions(&self) -> &Vec<client::Subscription> { /* 原样 */ }
    pub async fn example(root_paths: impl IntoIterator<Item = &Path>, cx: &mut AsyncApp) -> Entity<Project> { /* 原样 */ }
    pub async fn test(fs: Arc<dyn Fs>, root_paths: impl IntoIterator<Item = &Path>, cx: &mut TestAppContext) -> Entity<Project> { /* 原样 */ }
    pub async fn test_with_worktree_trust(fs: Arc<dyn Fs>, root_paths: impl IntoIterator<Item = &Path>, cx: &mut TestAppContext) -> Entity<Project> { /* 原样 */ }
    async fn test_project(fs: Arc<dyn Fs>, root_paths: impl IntoIterator<Item = &Path>, init_worktree_trust: bool, cx: &mut TestAppContext) -> Entity<Project> { /* 原样 */ }
    pub fn mark_as_collab_for_testing(&mut self) { /* 原样 */ }
    pub fn add_test_remote_worktree(&mut self, abs_path: &str, cx: &mut Context<Self>) -> Entity<worktree::Worktree> { /* 原样 */ }
    #[inline] pub fn has_open_buffer(&self, path: impl Into<ProjectPath>, cx: &App) -> bool { /* 原样 */ }
    pub fn open_local_buffer_with_lsp(&mut self, abs_path: impl AsRef<Path>, cx: &mut Context<Self>) -> gpui::Task<anyhow::Result<(Entity<language::Buffer>, lsp_store::OpenLspBufferHandle)>> { /* 原样 */ }
    pub fn open_buffer_with_lsp(&mut self, path: impl Into<ProjectPath>, cx: &mut Context<Self>) -> gpui::Task<anyhow::Result<(Entity<language::Buffer>, lsp_store::OpenLspBufferHandle)>> { /* 原样 */ }
}
