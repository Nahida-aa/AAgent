use std::sync::Arc;

use client::{AnyProtoClient, Client};
use gpui::{App, Context};

use super::Project;
use crate::{BufferStore, ContextServerStore, LspStore, SettingsObserver, TaskStore, WorktreeStore};
use crate::debugger::breakpoint_store::BreakpointStore;
use crate::debugger::dap_store::DapStore;
use crate::git_store::GitStore;

impl Project {
    pub fn init(client: &Arc<Client>, cx: &mut App) {
        // 原样搬入
    }
}
