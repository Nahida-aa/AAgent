//! Zed's Project: worktrees, buffers, LSP, DAP, search, collaboration, git.

pub mod agent_registry_store;
pub mod agent_server_store;
pub mod bookmark_store;
pub mod buffer_store;
pub mod color_extractor;
pub mod connection_manager;
pub mod context_server_store;
pub mod debounced_delay;
pub mod debugger;
pub mod git_store;
pub mod image_store;
pub mod lsp_command;
pub mod lsp_store;
pub mod manifest_tree;
pub mod prettier_store;
pub mod project_search;
pub mod project_settings;
pub mod search;
pub mod search_history;
pub mod task_inventory;
pub mod task_store;
pub mod telemetry_snapshot;
pub mod terminals;
pub mod toolchain_store;
pub mod trusted_worktrees;
pub mod worktree_store;
pub mod yarn;

mod constants;
mod directory;
mod event;
mod fuzzy;
mod group_key;
mod impls;
mod item;
mod path;
mod project;
mod protocol_helpers;
mod settings;
mod toast;
mod types;

#[cfg(test)]
mod tests;

// ---- 子模块 re-export ----
pub use agent_registry_store::{AgentRegistryStore, RegistryAgent};
pub use agent_server_store::{AgentId, AgentServerStore, AgentServersUpdated, ExternalAgentSource};
pub use buffer_store::ProjectTransaction;
pub use constants::{CURRENT_PROJECT_FEATURES, MAX_PROJECT_SEARCH_HISTORY_SIZE};
#[cfg(feature = "test-support")]
pub use constants::DEFAULT_COMPLETION_CONTEXT;
pub use directory::{DirectoryItem, DirectoryLister};
pub use event::{Event, OpenedBufferEvent};
// zed 这里同时导出 `PathMatchCandidateSetNucleoIter`（它把 fuzzy / fuzzy_nucleo
// 当两个 crate 各实现一遍）。我们的 `aa_gpui_fuzzy` 已经统一到 nucleo 终态，
// 只有一套 trait，故只保留 `PathMatchCandidateSetIter`。
pub use fuzzy::{Candidates, PathMatchCandidateSet, PathMatchCandidateSetIter};
pub use group_key::{ProjectGroupKey, path_suffix};
pub use item::ProjectItem;
pub use lsp_command::{CallHierarchyItem, IncomingCall, OutgoingCall};
pub use lsp_store::{
    DiagnosticSummary, InvalidationStrategy, LanguageServerLogType, LanguageServerProgress,
    LanguageServerPromptRequest, LanguageServerShowDocumentRequest, LanguageServerStatus,
    LanguageServerToQuery, LspStore, LspStoreEvent, ProgressToken,
    SERVER_PROGRESS_THROTTLE_TIMEOUT,
};
pub use path::{ProjectPath, ResolvedPath};
pub use project::*;
pub use project::{AgentLocation, ProjectGroupKey};
pub use settings::DisableAiSettings;
pub use task_inventory::{
    BasicContextProvider, ContextProviderWithTasks, DebugScenarioContext, GIT_COMMAND_TASK_TAG,
    Inventory, TaskContexts, TaskSourceKind,
};
pub use toast::ToastLink;
pub use toolchain_store::{ToolchainStore, Toolchains};
pub use types::*;

// ---- 保持原有 re-export ----
pub use environment::ProjectEnvironment;
pub use environment::ProjectEnvironmentEvent;
pub use git_store::{
    ConflictRegion, ConflictSet, ConflictSetSnapshot, ConflictSetUpdate,
    git_traversal::{ChildEntriesGitIter, GitEntry, GitEntryRef, GitTraversal},
    is_submodule_git_dir, linked_worktree_short_name, repo_identity_path,
    repo_identity_path_if_local, worktrees_directory_for_repo,
};
pub use image_store::{ImageItem, ImageStore};
pub use manifest_tree::{ManifestProvidersStore, ManifestTree};
pub use project_search::{Search, SearchResults};
pub use project_settings::{ProjectSettings, SettingsObserver, SettingsObserverEvent};
pub use snippet_provider;
pub use worktree_store::WorktreePaths;

// 外部 crate 的 re-export
pub use fs::*;
pub use language::Location;
#[cfg(any(test, feature = "test-support"))]
pub use prettier::FORMAT_SUFFIX as TEST_PRETTIER_FORMAT_SUFFIX;
#[cfg(any(test, feature = "test-support"))]
pub use prettier::RANGE_FORMAT_SUFFIX as TEST_PRETTIER_RANGE_FORMAT_SUFFIX;
pub use worktree::{
    Entry, EntryKind, FS_WATCH_LATENCY, File, LocalWorktree, PathChange, ProjectEntryId,
    UpdatedEntriesSet, UpdatedGitRepositoriesSet, Worktree, WorktreeId, WorktreeSettings,
    discover_root_repo_common_dir,
};
