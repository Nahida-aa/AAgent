use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use client::Client;
use gpui::{App, AppContext as _, Context, Entity, TaskExt as _};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use util::rel_path::RelPath;

use super::state::ProjectClientState;
use super::{LocalProjectFlags, Project};
use crate::constants::{CURRENT_PROJECT_FEATURES, DEFAULT_COMPLETION_CONTEXT};
use crate::directory::DirectoryLister;
use crate::lsp_store::LspStore;
use crate::settings::DisableAiSettings;
use crate::types::*;
use crate::{Event, ProjectEnvironmentEvent, ToastLink};

impl Project {
    pub fn local(
        client: Arc<Client>,
        node: NodeRuntime,
        user_store: Entity<client::UserStore>,
        languages: Arc<LanguageRegistry>,
        fs: Arc<dyn Fs>,
        env: Option<HashMap<String, String>>,
        flags: LocalProjectFlags,
        cx: &mut App,
    ) -> Entity<Self> {
        // 原样搬入
    }

    pub fn remote(
        remote: Entity<remote::RemoteClient>,
        client: Arc<Client>,
        node: NodeRuntime,
        user_store: Entity<client::UserStore>,
        languages: Arc<LanguageRegistry>,
        fs: Arc<dyn Fs>,
        init_worktree_trust: bool,
        cx: &mut App,
    ) -> Entity<Self> {
        // 原样搬入
    }

    #[cfg(feature = "test-support")]
    pub async fn example(
        root_paths: impl IntoIterator<Item = &std::path::Path>,
        cx: &mut gpui::AsyncApp,
    ) -> Entity<Project> {
        // 原样搬入
    }
}
