//! Worktree: local or remote file tree with scanning, git, and ignore support.

use ::ignore::gitignore::{Gitignore, GitignoreBuilder};
use crate::{IgnoreKind, IgnoreStack, WorktreeId, WorktreeSettings};
use aa_clock::ReplicaId;
use aa_gpui_fuzzy::CharBag;
use anyhow::{Context as _, Result, anyhow};
use async_channel::{self, Sender};
use collections::{BTreeMap, HashMap, HashSet, VecDeque};
use encoding_rs::Encoding;
use fs::{Fs, MTime, PathEvent, PathEventKind, Watcher};
use futures::{
    FutureExt as _, Stream, StreamExt,
    channel::{
        mpsc::{self, UnboundedSender},
        oneshot,
    },
    select_biased, stream,
    task::Poll,
};
use futures_lite::future::yield_now;
use ::git::{
    BISECT_LOG, COMMIT_MESSAGE, DOT_GIT, FETCH_HEAD, FSMONITOR_DAEMON, GC_PID, GITIGNORE,
    HOOKS_DIR, INFO_DIR, LFS_DIR, LOGS_DIR, LOGS_REF_STASH, OBJECTS_DIR, ORIG_HEAD,
    REBASE_APPLY_DIR, REBASE_MERGE_DIR, REFS_DIR, REFTABLE_DIR, REPO_EXCLUDE, SEQUENCER_DIR,
    status::GitSummary,
};
use gpui::{
    App, AppContext as _, AsyncApp, BackgroundExecutor, Context, Entity, EventEmitter, Priority,
    Task,
};
use language::{
    ByteContent, DiskState, FILE_ANALYSIS_BYTES, analyze_byte_content, decode_text, encode_text,
};
use parking_lot::Mutex;
use paths::{local_settings_folder_name, local_vscode_folder_name};
use postage::{
    barrier,
    prelude::{Sink as _, Stream as _},
};
pub(crate) use rpc::proto::split_worktree_update;
use rpc::AnyProtoClient;
use settings::{Settings, SettingsLocation, SettingsStore};
use smallvec::{SmallVec, smallvec};
use std::{
    any::Any,
    borrow::Borrow as _,
    cmp::Ordering,
    collections::hash_map,
    convert::TryFrom,
    ffi::OsStr,
    fmt,
    future::Future,
    io::Read,
    mem::{self},
    ops::{Deref, DerefMut, Range},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    time::{Duration, Instant},
};
use sum_tree::{Bias, Dimensions, Edit, KeyedItem, SeekTarget, SumTree, Summary, TreeMap, TreeSet};
use text::{LineEnding, Rope};
use util::{
    ResultExt, maybe,
    paths::{PathMatcher, PathStyle, SanitizedPath, home_dir},
    rel_path::RelPath,
};

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
pub(crate) use repo::{LocalRepositoryEntry, ScanState, UpdateObservationState};
pub use snapshot::Snapshot;
pub use traversal::{ChildEntriesIter, ChildEntriesOptions, PathTarget, Traversal};
pub(crate) use traversal::TraversalTarget;
pub use worktree::Worktree;

pub(crate) use git::{
    discover_ancestor_git_repo, discover_git_paths, discover_root_repo_common_dir,
    discover_root_repo_metadata, is_dot_git, watch_dir_tree, watch_git_dir_subdirectories,
};
pub(crate) use loading::{decode_file_text, decode_file_text_to_rope};
pub(crate) use scanner::{
    BackgroundScanner, BackgroundScannerPhase, BackgroundScannerState, EventRoot, RemovedEntries,
    ScanJob, UpdateIgnoreStatusJob, build_diff, char_bag_for_path, is_beyond_scan_depth,
    merge_event_roots, swap_to_front,
};
pub(crate) use gitignore::{build_gitignore, build_gitignore_with_root};
pub(crate) use summaries::{
    EntrySummary, PathEntry, PathEntrySummary, PathKey, PathProgress, PathSummary,
    TraversalProgress,
};
pub(crate) use watch::NullWatcher;
