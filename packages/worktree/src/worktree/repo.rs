use super::*;
use postage::watch;

/// This path corresponds to the 'content path' of a repository in relation
/// to Zed's project root.
/// In the majority of the cases, this is the folder that contains the .git folder.
/// But if a sub-folder of a git repository is opened, this corresponds to the
/// project root and the .git folder is located in a parent directory.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum WorkDirectory {
    InProject {
        relative_path: Arc<RelPath>,
    },
    AboveProject {
        absolute_path: Arc<Path>,
        location_in_repo: Arc<Path>,
    },
}

impl WorkDirectory {
    pub(crate) fn path_key(&self) -> PathKey {
        match self {
            WorkDirectory::InProject { relative_path } => PathKey(relative_path.clone()),
            WorkDirectory::AboveProject { .. } => PathKey(RelPath::empty_arc()),
        }
    }

    /// Returns true if the given path is a child of the work directory.
    ///
    /// Note that the path may not be a member of this repository, if there
    /// is a repository in a directory between these two paths
    /// external .git folder in a parent folder of the project root.
    #[track_caller]
    pub fn directory_contains(&self, path: &RelPath) -> bool {
        match self {
            WorkDirectory::InProject { relative_path } => path.starts_with(relative_path),
            WorkDirectory::AboveProject { .. } => true,
        }
    }
}

impl Default for WorkDirectory {
    fn default() -> Self {
        Self::InProject {
            relative_path: Arc::from(RelPath::empty()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LocalRepositoryEntry {
    pub(crate) work_directory_id: ProjectEntryId,
    pub(crate) work_directory: WorkDirectory,
    pub(crate) work_directory_abs_path: Arc<Path>,
    pub(crate) git_dir_scan_id: usize,
    /// Absolute path to the original .git entry that caused us to create this repository.
    ///
    /// This is normally a directory, but may be a "gitfile" that points to a directory elsewhere
    /// (whose path we then store in `repository_dir_abs_path`).
    pub(crate) dot_git_abs_path: Arc<Path>,
    /// Absolute path to the "commondir" for this repository.
    ///
    /// This is always a directory. For a normal repository, this is the same as
    /// `dot_git_abs_path`. For a linked worktree, this is the main repo's `.git`
    /// directory (resolved from the worktree's `commondir` file). For a submodule,
    /// this equals `repository_dir_abs_path` (submodules don't have a `commondir`
    /// file).
    pub(crate) common_dir_abs_path: Arc<Path>,
    /// Absolute path to the directory holding the repository's state.
    ///
    /// For a normal repository, this is a directory and coincides with `dot_git_abs_path` and
    /// `common_dir_abs_path`. For a submodule or worktree, this is some subdirectory of the
    /// commondir like `/project/.git/modules/foo`.
    pub(crate) repository_dir_abs_path: Arc<Path>,
}

impl sum_tree::Item for LocalRepositoryEntry {
    type Summary = PathSummary<sum_tree::NoSummary>;

    fn summary(&self, _: <Self::Summary as Summary>::Context<'_>) -> Self::Summary {
        PathSummary {
            max_path: self.work_directory.path_key().0,
            item_summary: sum_tree::NoSummary,
        }
    }
}

impl KeyedItem for LocalRepositoryEntry {
    type Key = PathKey;

    fn key(&self) -> Self::Key { self.work_directory.path_key() }
}

impl Deref for LocalRepositoryEntry {
    type Target = WorkDirectory;

    fn deref(&self) -> &Self::Target { &self.work_directory }
}

pub(crate) enum ScanState {
    Started,
    Updated {
        snapshot: LocalSnapshot,
        changes: UpdatedEntriesSet,
        barrier: SmallVec<[barrier::Sender; 1]>,
        scanning: bool,
    },
    RootUpdated {
        new_path: Arc<SanitizedPath>,
    },
    RootDeleted,
}

pub(crate) struct UpdateObservationState {
    snapshots_tx: mpsc::UnboundedSender<(LocalSnapshot, UpdatedEntriesSet)>,
    resume_updates: watch::Sender<()>,
    _maintain_remote_snapshot: Task<Option<()>>,
}
