use std::ops::Range;

use crate::git_store::{Repository, RepositoryId};

use super::*;

use ::git::status::FileStatus;
use anyhow::{Context as _, Result, anyhow};
use buffer_diff::BufferDiff;
use text::Anchor;

// project/git.rs
impl Project {
    pub fn repositories<'a>(&self, cx: &'a App) -> &'a HashMap<RepositoryId, Entity<Repository>> {
        self.git_store.read(cx).repositories()
    }

    // 它旁边的兄弟方法
    pub fn active_repository(&self, cx: &App) -> Option<Entity<Repository>> {
        self.git_store.read(cx).active_repository()
    }

    pub fn status_for_buffer_id(&self, buffer_id: BufferId, cx: &App) -> Option<FileStatus> {
        self.git_store.read(cx).status_for_buffer_id(buffer_id, cx)
    }

    /// Stages the worktree changes covered by `worktree_ranges` (in the worktree
    /// buffer's coordinates), acting on the given unstaged diff. Used by both the
    /// unstaged-changes view and the uncommitted (gutter) controls.
    pub fn stage_hunks(
        &mut self,
        buffer: Entity<Buffer>,
        unstaged_diff: Entity<BufferDiff>,
        worktree_ranges: Vec<Range<Anchor>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if self.is_disconnected(cx) {
            return Err(anyhow!(ErrorCode::Disconnected));
        }
        self.git_store.update(cx, |git_store, cx| {
            git_store.stage_hunks(buffer, unstaged_diff, worktree_ranges, cx)
        })
    }

    /// Unstages the worktree changes covered by `worktree_ranges` (in the worktree
    /// buffer's coordinates), acting on the given uncommitted diff. Used by the
    /// uncommitted (gutter) controls.
    pub fn unstage_uncommitted_hunks(
        &mut self,
        buffer: Entity<Buffer>,
        uncommitted_diff: Entity<BufferDiff>,
        worktree_ranges: Vec<Range<Anchor>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if self.is_disconnected(cx) {
            return Err(anyhow!(ErrorCode::Disconnected));
        }
        self.git_store.update(cx, |git_store, cx| {
            git_store.unstage_uncommitted_hunks(buffer, uncommitted_diff, worktree_ranges, cx)
        })
    }

    /// Unstages the staged changes covered by `index_ranges` (in the index
    /// buffer's coordinates), acting on the given staged diff. Used by the
    /// staged-changes view.
    pub fn unstage_staged_hunks(
        &mut self,
        staged_diff: Entity<BufferDiff>,
        index_ranges: Vec<Range<Anchor>>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if self.is_disconnected(cx) {
            return Err(anyhow!(ErrorCode::Disconnected));
        }
        self.git_store.update(cx, |git_store, cx| {
            git_store.unstage_staged_hunks(staged_diff, index_ranges, cx)
        })
    }
    // 当用户在编辑器中选中若干行，想分享一个指向这些行的永久链接时，UI 层会调用 Project::get_permalink_to_line
    pub fn get_permalink_to_line(
        &self,
        buffer: &Entity<Buffer>,
        selection: Range<u32>,
        cx: &mut App,
    ) -> Task<Result<url::Url>> {
        self.git_store.update(cx, |git_store, cx| {
            git_store.get_permalink_to_line(buffer, selection, cx)
        })
    }
    // 当用户想分享某个文件的永久链接，而不关心具体行号时使用
    pub fn get_file_permalink(
        &self,
        project_path: &ProjectPath,
        cx: &mut App,
    ) -> Task<Result<url::Url>> {
        self.git_store.update(cx, |git_store, cx| {
            git_store.get_file_permalink(project_path, cx)
        })
    }

}
