use anyhow::{Context as _, Result};
use client::TypedEnvelope;
use futures::StreamExt;
use gpui::{AppContext as _, AsyncApp, Entity};
use itertools::Itertools;

use super::Project;
use crate::ProjectPath;

impl Project {
    pub(crate) async fn handle_unshare_project(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_add_collaborator(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_update_project_collaborator(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_remove_collaborator(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_update_project(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_toast(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_telemetry_event(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_hide_toast(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_update_worktree(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_trust_worktrees(...) -> Result<proto::Ack> { /* 原样 */ }
    pub(crate) async fn handle_restrict_worktrees(...) -> Result<proto::Ack> { /* 原样 */ }
    pub(crate) async fn handle_find_search_candidates_chunk(...) -> Result<proto::Ack> { /* 原样 */ }
    pub(crate) async fn handle_find_search_candidates_cancel(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_create_buffer_for_peer(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_create_image_for_peer(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_create_file_for_peer(...) -> Result<()> { /* 原样 */ }
    pub(crate) async fn handle_search_candidate_buffers(...) -> Result<proto::Ack> { /* 原样 */ }
    pub(crate) async fn handle_open_buffer_by_id(...) -> Result<proto::OpenBufferResponse> { /* 原样 */ }
    pub(crate) async fn handle_open_buffer_by_path(...) -> Result<proto::OpenBufferResponse> { /* 原样 */ }
    pub(crate) async fn handle_open_new_buffer(...) -> Result<proto::OpenBufferResponse> { /* 原样 */ }
}
