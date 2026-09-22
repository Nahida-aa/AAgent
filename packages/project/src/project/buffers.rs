use super::*;

use std::sync::Arc;

use crate::buffer_store::BufferStore;

use std::collections::HashSet;

use anyhow::{Context as _, Result, anyhow};
use futures::{
    FutureExt as _, StreamExt as _,
    channel::mpsc::UnboundedReceiver,
    future::try_join_all,
};
use gpui::{App, AsyncApp, Context, Entity, Task, WeakEntity};
use itertools::Either;
use language::{
    Buffer, BufferEvent, BufferId, Capability, DiskState, Rope, ToOffset,
    proto::split_operations,
};
use rpc::ErrorCode;

use super::state::BufferOrderedMessage;
use super::Project;
use crate::buffer_store::{BufferStoreEvent, ProjectTransaction};
use crate::ProjectPath;

impl Project {
    pub fn create_buffer(
        &mut self,
        language: Option<Arc<language::Language>>,
        project_searchable: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> { /* 原样 */ }

    pub fn create_local_buffer(
        &mut self,
        text: &str,
        language: Option<Arc<language::Language>>,
        project_searchable: bool,
        cx: &mut Context<Self>,
    ) -> Entity<Buffer> { /* 原样 */ }

    pub fn open_path(
        &mut self,
        path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Task<Result<(Option<worktree::ProjectEntryId>, Entity<Buffer>)>> { /* 原样 */ }

    pub fn open_local_buffer(
        &mut self,
        abs_path: impl AsRef<std::path::Path>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> { /* 原样 */ }

    pub fn open_buffer(
        &mut self,
        path: impl Into<ProjectPath>,
        cx: &mut App,
    ) -> Task<Result<Entity<Buffer>>> { /* 原样 */ }

    pub fn open_buffer_by_id(
        &mut self,
        id: BufferId,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Buffer>>> { /* 原样 */ }

    pub fn save_buffers(
        &self,
        buffers: HashSet<Entity<Buffer>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> { /* 原样 */ }

    pub fn save_buffer(&self, buffer: Entity<Buffer>, cx: &mut Context<Self>) -> Task<Result<()>> { /* 原样 */ }
    pub fn save_buffer_as(&mut self, buffer: Entity<Buffer>, path: ProjectPath, cx: &mut Context<Self>) -> Task<Result<()>> { /* 原样 */ }
    pub fn get_open_buffer(&self, path: &ProjectPath, cx: &App) -> Option<Entity<Buffer>> { /* 原样 */ }
    pub fn dirty_buffers<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = ProjectPath> + 'a { /* 原样 */ }
    pub fn reload_buffers(
        &self,
        buffers: HashSet<Entity<Buffer>>,
        push_to_history: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectTransaction>> { /* 原样 */ }

    fn register_buffer(&mut self, buffer: &Entity<Buffer>, cx: &mut Context<Self>) -> Result<()> { /* 原样 */ }

    pub(crate) fn on_buffer_store_event(
        &mut self,
        _: Entity<BufferStore>,
        event: &BufferStoreEvent,
        cx: &mut Context<Self>,
    ) { /* 原样 */ }

    pub(crate) fn on_buffer_event(
        &mut self,
        buffer: Entity<Buffer>,
        event: &BufferEvent,
        cx: &mut Context<Self>,
    ) -> Option<()> { /* 原样 */ }

    fn request_buffer_diff_recalculation(
        &mut self,
        buffer: &Entity<Buffer>,
        cx: &mut Context<Self>,
    ) { /* 原样 */ }

    fn recalculate_buffer_diffs(&mut self, cx: &mut Context<Self>) -> Task<()> { /* 原样 */ }

    async fn send_buffer_ordered_messages(
        project: WeakEntity<Self>,
        rx: UnboundedReceiver<BufferOrderedMessage>,
        cx: &mut AsyncApp,
    ) -> Result<()> { /* 原样 */ }

    fn enqueue_buffer_ordered_message(&mut self, message: BufferOrderedMessage) -> Result<()> { /* 原样 */ }

    fn synchronize_remote_buffers(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> { /* 原样 */ }
}
