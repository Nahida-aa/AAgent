use anyhow::{Context as _, Result, anyhow};
use gpui::{AsyncApp, Context};

use super::state::ProjectClientState;
use super::Project;

impl Project {
    pub(crate) fn release(&mut self, cx: &mut gpui::App) { /* 原样 */ }
    pub(crate) fn shared(&mut self, project_id: u64, cx: &mut Context<Self>) -> Result<()> { /* 原样 */ }
    pub(crate) fn reshared(&mut self, message: proto::ResharedProject, cx: &mut Context<Self>) -> Result<()> { /* 原样 */ }
    pub(crate) fn rejoined(
        &mut self,
        message: proto::RejoinedProject,
        message_id: u32,
        cx: &mut Context<Self>,
    ) -> Result<()> { /* 原样 */ }
    #[inline] pub(crate) fn unshare(&mut self, cx: &mut Context<Self>) -> Result<()> { /* 原样 */ }
    pub(crate) fn unshare_internal(&mut self, cx: &mut gpui::App) -> Result<()> { /* 原样 */ }
    pub(crate) fn disconnected_from_host(&mut self, cx: &mut Context<Self>) { /* 原样 */ }
    pub(crate) fn disconnected_from_host_internal(&mut self, cx: &mut gpui::App) { /* 原样 */ }
    pub(crate) fn set_role(&mut self, role: proto::ChannelRole, cx: &mut Context<Self>) { /* 原样 */ }
    #[inline] pub(crate) fn close(&mut self, cx: &mut Context<Self>) { /* 原样 */ }
    #[inline] pub(crate) fn is_disconnected(&self, cx: &gpui::App) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn capability(&self) -> language::Capability { /* 原样 */ }
    #[inline] pub(crate) fn is_read_only(&self, cx: &gpui::App) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_local(&self) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_via_remote_server(&self) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_via_collab(&self) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_remote(&self) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_via_wsl_with_host_interop(&self, cx: &gpui::App) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn is_via_wsl(&self, cx: &gpui::App) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn remote_id(&self) -> Option<u64> { /* 原样 */ }
    #[inline] pub(crate) fn replica_id(&self) -> clock::ReplicaId { /* 原样 */ }
    #[inline] pub(crate) fn is_shared(&self) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn supports_terminal(&self, _cx: &gpui::App) -> bool { /* 原样 */ }
    #[inline] pub(crate) fn remote_connection_state(&self, cx: &gpui::App) -> Option<remote::ConnectionState> { /* 原样 */ }
    #[inline] pub(crate) fn remote_connection_options(&self, cx: &gpui::App) -> Option<remote::RemoteConnectionOptions> { /* 原样 */ }
    pub(crate) fn reveal_path(&self, path: &std::path::Path, cx: &mut Context<Self>) { /* 原样 */ }
    pub(crate) fn set_active_path(
        &mut self,
        entry: Option<crate::ProjectPath>,
        cx: &mut Context<Self>,
    ) { /* 原样 */ }
    pub(crate) fn disable_worktree_scanner(&mut self, cx: &mut Context<Self>) { /* 原样 */ }
}
