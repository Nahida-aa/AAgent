//! Worktree: local or remote file tree with scanning, git, and ignore support.

use anyhow::anyhow;
use futures::select_biased;
use smallvec::smallvec;
use util::maybe;

mod constants;
mod entry;
mod event;
mod file;
mod git;
mod gitignore;
mod loading;
mod local;
mod local_snapshot;
mod model_handle;
mod proto;
mod remote;
mod repo;
mod scanner;
mod snapshot;
mod summaries;
mod traversal;
mod watch;
mod worktree;

#[cfg(feature = "test-support")]
mod test_support;

pub use constants::{FS_WATCH_LATENCY, ROOT_PATH_CHECK_INTERVAL};
pub(crate) use constants::STREAM_BLOCK_BYTES;
pub use entry::{
    Entry, EntryKind, PathChange, ProjectEntryId, UpdatedEntriesSet, UpdatedGitRepositoriesSet,
    UpdatedGitRepository,
};
pub use event::{CreatedEntry, Event, LoadedBinaryFile, LoadedFile};
pub use file::File;
pub use local::{LocalWorktree, PathPrefixScanRequest, ScanRequest};
pub use local_snapshot::LocalSnapshot;
pub use model_handle::WorktreeModelHandle;
pub use remote::RemoteWorktree;
pub use repo::WorkDirectory;
pub(crate) use repo::LocalRepositoryEntry;
use rpc::proto;
pub use snapshot::Snapshot;
pub use traversal::{ChildEntriesIter, ChildEntriesOptions, Traversal};
pub use worktree::Worktree;

pub(crate) use git::{
    discover_ancestor_git_repo, discover_git_paths, discover_root_repo_common_dir,
};
pub(crate) use loading::{decode_file_text, decode_file_text_to_rope};
pub(crate) use scanner::diff::{EventRoot, build_diff, merge_event_roots};
pub(crate) use scanner::{
    BackgroundScanner, BackgroundScannerState, ScanJob, UpdateIgnoreStatusJob,
};
pub(crate) use summaries::{
    EntrySummary, PathEntry, PathEntrySummary, PathKey, PathProgress, PathSummary,
    TraversalProgress,
};
pub(crate) use watch::NullWatcher;
