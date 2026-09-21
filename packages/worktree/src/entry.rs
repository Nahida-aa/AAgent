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
