//! The `Project` entity and its associated state.

mod ai;
mod buffers;
mod collab;
mod constructors;
mod dap;
mod diagnostics;
mod events;
mod git;
mod helpers;
mod images;
mod init;
mod lifecycle;
mod lsp;
mod lsp_events;
mod lsp_rpc;
mod paths;
mod rpc;
mod search;
pub mod state;
mod toolchains;
mod worktrees;

#[cfg(feature = "test-support")]
mod test_support;

pub use state::{
    AgentLocation, BufferOrderedMessage, DebugAdapterClientState, DownloadingFile,
    EntitySubscription, LocalProjectFlags, ProjectClientState, RemotelyCreatedModelGuard,
    RemotelyCreatedModels,
};

// ---- 供 project/ 各子模块经 `use super::*;` 取用 ----
// zed 的 project.rs 是 crate 根，子模块用的裸名天然可见；拆分后在这里统一转发。
// 这些供 project/ 各子模块经 `use super::*;` 取用。
// glob 导入不会提升可见性，所以必须是 `pub use`（而非 pub(crate)）。
// 注意：这里**不要**转发 `proto`——子模块自己 `use ::rpc::proto;`。
// 否则 `use super::*` 会把 `proto` 带进子模块，让文件里的 `::rpc::proto::X`
// 被解析成 `proto::proto::X`。
pub use crate::Event;
pub use crate::ProjectPath;
pub use crate::item::ProjectItem;
pub use crate::lsp_store::{LanguageServerToQuery, LspStoreEvent, ProgressToken};
pub use crate::search::{SearchQuery, SearchResult};
pub use crate::search_history::SearchHistory;
pub use ::rpc::ErrorCode;
pub use client::Collaborator;
pub use fs::{Fs, TrashId};
pub use gpui::Context;
pub use language::{Capability, LanguageServerId};
pub use remote::RemoteConnectionOptions;
pub use settings::WorktreeId;
pub use util::rel_path::RelPath;
pub use worktree::ProjectEntryId;

use collections::IndexSet;
use collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) use crate::buffer_store::BufferStore;
pub(crate) use crate::context_server_store::ContextServerStore;
pub(crate) use crate::debounced_delay::DebouncedDelay;
pub(crate) use crate::debugger::{
    breakpoint_store::{ActiveStackFrame, BreakpointStore},
    dap_store::DapStore,
    session::Session,
};
pub(crate) use crate::image_store::{ImageItem, ImageStore};
pub(crate) use crate::lsp_store::LspStore;
pub(crate) use crate::task_store::TaskStore;
pub(crate) use crate::terminals::Terminals;
pub(crate) use crate::toolchain_store::ToolchainStore;
pub(crate) use crate::worktree_store::{WorktreeIdCounter, WorktreeStore, WorktreeStoreEvent};
pub(crate) use ::dap::client::DebugAdapterClient;
pub(crate) use ::dap::inline_value::VariableLookupKind;
pub(crate) use ::rpc::AnyProtoClient;
use client::{Client, UserStore};
pub(crate) use clock::ReplicaId;
use futures::channel::mpsc;
use gpui::{App, Entity, EventEmitter, SharedString, Task, WeakEntity};
use itertools::{Either, Itertools};
use language::{Buffer, BufferId, Language, LanguageRegistry, Toolchain, ToolchainMetadata};
use node_runtime::NodeRuntime;
use parking_lot::Mutex;
use remote::RemoteClient;
pub(crate) use settings::SettingsStore;
use snippet_provider::SnippetProvider;
use util::path_list::PathList;

use crate::bookmark_store::BookmarkStore;
use crate::git_store::GitStore;
use crate::project_search::SearchResultsHandle;
use crate::trusted_worktrees::{PathTrust, RemoteHostLocation, TrustedWorktrees};
use crate::worktree_store::WorktreePaths;
use crate::{ProjectEnvironment, SettingsObserver};

pub struct Project {
    pub(crate) active_entry: Option<ProjectEntryId>,
    pub(crate) buffer_ordered_messages_tx: mpsc::UnboundedSender<BufferOrderedMessage>,
    pub(crate) languages: Arc<LanguageRegistry>,
    pub(crate) dap_store: Entity<DapStore>,
    pub(crate) agent_server_store: Entity<crate::AgentServerStore>,

    pub(crate) bookmark_store: Entity<BookmarkStore>,
    pub(crate) breakpoint_store: Entity<BreakpointStore>,
    pub(crate) collab_client: Arc<client::Client>,
    pub(crate) join_project_response_message_id: u32,
    pub(crate) task_store: Entity<TaskStore>,
    pub(crate) user_store: Entity<UserStore>,
    pub(crate) fs: Arc<dyn Fs>,
    pub(crate) remote_client: Option<Entity<RemoteClient>>,
    pub(crate) client_state: ProjectClientState,
    pub(crate) git_store: Entity<GitStore>,
    pub(crate) collaborators: HashMap<::rpc::proto::PeerId, Collaborator>,
    pub(crate) client_subscriptions: Vec<client::Subscription>,
    pub(crate) worktree_store: Entity<WorktreeStore>,
    pub(crate) buffer_store: Entity<BufferStore>,
    pub(crate) context_server_store: Entity<ContextServerStore>,
    pub(crate) image_store: Entity<ImageStore>,
    pub(crate) lsp_store: Entity<LspStore>,
    pub(crate) _subscriptions: Vec<gpui::Subscription>,
    pub(crate) buffers_needing_diff: HashSet<WeakEntity<Buffer>>,
    pub(crate) git_diff_debouncer: DebouncedDelay<Self>,
    pub(crate) remotely_created_models: Arc<Mutex<RemotelyCreatedModels>>,
    pub(crate) terminals: Terminals,
    pub(crate) node: Option<NodeRuntime>,
    pub(crate) search_history: SearchHistory,
    pub(crate) search_included_history: SearchHistory,
    pub(crate) search_excluded_history: SearchHistory,
    pub(crate) snippets: Entity<SnippetProvider>,
    pub(crate) environment: Entity<ProjectEnvironment>,
    pub(crate) settings_observer: Entity<SettingsObserver>,
    pub(crate) toolchain_store: Option<Entity<ToolchainStore>>,
    pub(crate) agent_location: Option<AgentLocation>,
    pub(crate) downloading_files: Arc<Mutex<HashMap<(WorktreeId, String), DownloadingFile>>>,
    pub(crate) last_worktree_paths: WorktreePaths,
}

impl EventEmitter<Event> for Project {}

// ---------- 简单访问器（很多都是 #[inline]） ----------

impl Project {
    #[inline]
    pub fn dap_store(&self) -> Entity<DapStore> { self.dap_store.clone() }
    #[inline]
    pub fn bookmark_store(&self) -> Entity<BookmarkStore> { self.bookmark_store.clone() }
    #[inline]
    pub fn breakpoint_store(&self) -> Entity<BreakpointStore> { self.breakpoint_store.clone() }
    #[inline]
    pub fn lsp_store(&self) -> Entity<LspStore> { self.lsp_store.clone() }
    #[inline]
    pub fn worktree_store(&self) -> Entity<WorktreeStore> { self.worktree_store.clone() }
    #[inline]
    pub fn context_server_store(&self) -> Entity<ContextServerStore> {
        self.context_server_store.clone()
    }
    #[inline]
    pub fn buffer_store(&self) -> &Entity<BufferStore> { &self.buffer_store }
    #[inline]
    pub fn git_store(&self) -> &Entity<GitStore> { &self.git_store }
    #[inline]
    pub fn agent_server_store(&self) -> &Entity<crate::AgentServerStore> {
        &self.agent_server_store
    }
    #[inline]
    pub fn task_store(&self) -> &Entity<TaskStore> { &self.task_store }
    #[inline]
    pub fn snippets(&self) -> &Entity<SnippetProvider> { &self.snippets }
    #[inline]
    pub fn languages(&self) -> &Arc<LanguageRegistry> { &self.languages }
    #[inline]
    pub fn client(&self) -> Arc<Client> { self.collab_client.clone() }
    #[inline]
    pub fn remote_client(&self) -> Option<Entity<RemoteClient>> { self.remote_client.clone() }
    #[inline]
    pub fn user_store(&self) -> Entity<UserStore> { self.user_store.clone() }
    #[inline]
    pub fn node_runtime(&self) -> Option<&NodeRuntime> { self.node.as_ref() }
    #[inline]
    pub fn fs(&self) -> &Arc<dyn Fs> { &self.fs }
    #[inline]
    pub fn environment(&self) -> &Entity<ProjectEnvironment> { &self.environment }
    #[inline]
    pub fn active_entry(&self) -> Option<ProjectEntryId> { self.active_entry }
    #[inline]
    pub fn collaborators(&self) -> &HashMap<::rpc::proto::PeerId, Collaborator> {
        &self.collaborators
    }
    #[inline]
    pub fn host(&self) -> Option<&Collaborator> { self.collaborators.values().find(|c| c.is_host) }
    #[inline]
    pub fn opened_buffers(&self, cx: &App) -> Vec<Entity<Buffer>> {
        self.buffer_store.read(cx).buffers().collect()
    }
    #[inline]
    pub fn buffer_for_id(&self, remote_id: BufferId, cx: &App) -> Option<Entity<Buffer>> {
        self.buffer_store.read(cx).get(remote_id)
    }
}
