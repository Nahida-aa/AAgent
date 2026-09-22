use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: ProjectEntryId,
    pub kind: EntryKind,
    pub path: Arc<RelPath>,
    pub inode: u64,
    pub mtime: Option<MTime>,

    pub canonical_path: Option<Arc<Path>>,
    /// Whether this entry is ignored by Git.
    ///
    /// We only scan ignored entries once the directory is expanded and
    /// exclude them from searches.
    pub is_ignored: bool,

    /// Whether this entry is hidden or inside hidden directory.
    ///
    /// We only scan hidden entries once the directory is expanded.
    pub is_hidden: bool,

    /// Whether this entry is always included in searches.
    ///
    /// This is used for entries that are always included in searches, even
    /// if they are ignored by git. Overridden by file_scan_exclusions.
    pub is_always_included: bool,

    /// Whether this entry's canonical path is outside of the worktree.
    /// This means the entry is only accessible from the worktree root via a
    /// symlink.
    ///
    /// We only scan entries outside of the worktree once the symlinked
    /// directory is expanded.
    pub is_external: bool,

    /// Whether this entry is considered to be a `.env` file.
    pub is_private: bool,
    /// The entry's size on disk, in bytes.
    pub size: u64,
    pub char_bag: CharBag,
    pub is_fifo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    UnloadedDir,
    PendingDir,
    Dir,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathChange {
    Added,
    Removed,
    Updated,
    AddedOrUpdated,
    Loaded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatedGitRepository {
    /// ID of the repository's working directory.
    ///
    /// For a repo that's above the worktree root, this is the ID of the worktree root, and hence not unique.
    /// It's included here to aid the GitStore in detecting when a repository's working directory is renamed.
    pub work_directory_id: ProjectEntryId,
    pub old_work_directory_abs_path: Option<Arc<Path>>,
    pub new_work_directory_abs_path: Option<Arc<Path>>,
    /// For a normal git repository checkout, the absolute path to the .git directory.
    /// For a worktree, the absolute path to the worktree's subdirectory inside the .git directory.
    pub dot_git_abs_path: Option<Arc<Path>>,
    pub repository_dir_abs_path: Option<Arc<Path>>,
    pub common_dir_abs_path: Option<Arc<Path>>,
}

pub type UpdatedEntriesSet = Arc<[(Arc<RelPath>, ProjectEntryId, PathChange)]>;
pub type UpdatedGitRepositoriesSet = Arc<[UpdatedGitRepository]>;

impl Entry {
    pub(crate) fn new(
        path: Arc<RelPath>,
        metadata: &fs::Metadata,
        id: ProjectEntryId,
        root_char_bag: CharBag,
        canonical_path: Option<Arc<Path>>,
    ) -> Self {
        let char_bag = char_bag_for_path(root_char_bag, &path);
        Self {
            id,
            kind: if metadata.is_dir {
                EntryKind::PendingDir
            } else {
                EntryKind::File
            },
            path,
            inode: metadata.inode,
            mtime: Some(metadata.mtime),
            size: metadata.len,
            canonical_path,
            is_ignored: false,
            is_hidden: false,
            is_always_included: false,
            is_external: false,
            is_private: false,
            char_bag,
            is_fifo: metadata.is_fifo,
        }
    }

    pub fn is_created(&self) -> bool { self.mtime.is_some() }

    pub fn is_dir(&self) -> bool { self.kind.is_dir() }

    pub fn is_file(&self) -> bool { self.kind.is_file() }
}

impl EntryKind {
    pub fn is_dir(&self) -> bool {
        matches!(
            self,
            EntryKind::Dir | EntryKind::PendingDir | EntryKind::UnloadedDir
        )
    }

    pub fn is_unloaded(&self) -> bool { matches!(self, EntryKind::UnloadedDir) }

    pub fn is_file(&self) -> bool { matches!(self, EntryKind::File) }
}

impl sum_tree::Item for Entry {
    type Summary = EntrySummary;

    fn summary(&self, _cx: ()) -> Self::Summary {
        let non_ignored_count = if self.is_ignored && !self.is_always_included {
            0
        } else {
            1
        };
        let file_count;
        let non_ignored_file_count;
        if self.is_file() {
            file_count = 1;
            non_ignored_file_count = non_ignored_count;
        } else {
            file_count = 0;
            non_ignored_file_count = 0;
        }

        EntrySummary {
            max_path: self.path.clone(),
            count: 1,
            non_ignored_count,
            file_count,
            non_ignored_file_count,
            deferred_scan_dir_count: usize::from(
                self.kind == EntryKind::UnloadedDir
                    && !self.is_ignored
                    && !self.is_external
                    && !self.path.is_empty(),
            ),
        }
    }
}

impl sum_tree::KeyedItem for Entry {
    type Key = PathKey;

    fn key(&self) -> Self::Key { PathKey(self.path.clone()) }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectEntryId(pub(crate) usize);

impl ProjectEntryId {
    pub const MAX: Self = Self(usize::MAX);
    pub const MIN: Self = Self(usize::MIN);

    pub fn new(counter: &AtomicUsize) -> Self { Self(counter.fetch_add(1, SeqCst)) }

    pub fn from_proto(id: u64) -> Self { Self(id as usize) }

    pub fn to_proto(self) -> u64 { self.0 as u64 }

    pub fn from_usize(id: usize) -> Self { ProjectEntryId(id) }

    pub fn to_usize(self) -> usize { self.0 }
}
