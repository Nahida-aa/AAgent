use std::sync::Arc;

use aa_gpui_fuzzy::CharBag;
use settings::WorktreeId;
use sum_tree::SumTree;
use util::{
    paths::{PathMatcher, PathStyle, SanitizedPath},
    rel_path::RelPath,
};

use super::*;

#[derive(Clone)]
pub struct Snapshot {
    pub(crate) id: WorktreeId,
    /// The absolute path of the worktree root.
    pub(crate) abs_path: Arc<SanitizedPath>,
    pub(crate) path_style: PathStyle,
    pub(crate) root_name: Arc<RelPath>,
    pub(crate) root_char_bag: CharBag,
    pub(crate) entries_by_path: SumTree<Entry>,
    pub(crate) entries_by_id: SumTree<PathEntry>,
    pub(crate) root_repo_common_dir: Option<Arc<SanitizedPath>>,
    pub(crate) root_repo_is_linked_worktree: bool,

    /// A number that increases every time the worktree begins scanning
    /// a set of paths from the filesystem. This scanning could be caused
    /// by some operation performed on the worktree, such as reading or
    /// writing a file, or by an event reported by the filesystem.
    pub(crate) scan_id: usize,

    /// The latest scan id that has completed, and whose preceding scans
    /// have all completed. The current `scan_id` could be more than one
    /// greater than the `completed_scan_id` if operations are performed
    /// on the worktree while it is processing a file-system event.
    pub(crate) completed_scan_id: usize,
}

impl Snapshot {
    pub fn new(
        id: WorktreeId,
        root_name: Arc<RelPath>,
        abs_path: Arc<Path>,
        path_style: PathStyle,
    ) -> Self {
        Snapshot {
            id,
            abs_path: SanitizedPath::from_arc(abs_path),
            path_style,
            root_char_bag: root_name
                .as_unix_str()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect(),
            root_name,
            entries_by_path: Default::default(),
            entries_by_id: Default::default(),
            root_repo_common_dir: None,
            root_repo_is_linked_worktree: false,
            scan_id: 1,
            completed_scan_id: 0,
        }
    }

    pub fn id(&self) -> WorktreeId { self.id }

    // TODO:
    // Consider the following:
    //
    // ```rust
    // let abs_path: Arc<Path> = snapshot.abs_path(); // e.g. "C:\Users\user\Desktop\project"
    // let some_non_trimmed_path = Path::new("\\\\?\\C:\\Users\\user\\Desktop\\project\\main.rs");
    // // The caller perform some actions here:
    // some_non_trimmed_path.strip_prefix(abs_path);  // This fails
    // some_non_trimmed_path.starts_with(abs_path);   // This fails too
    // ```
    //
    // This is definitely a bug, but it's not clear if we should handle it here or not.
    pub fn abs_path(&self) -> &Arc<Path> { SanitizedPath::cast_arc_ref(&self.abs_path) }

    pub fn root_repo_common_dir(&self) -> Option<&Arc<Path>> {
        self.root_repo_common_dir
            .as_ref()
            .map(SanitizedPath::cast_arc_ref)
    }

    pub fn root_repo_is_linked_worktree(&self) -> bool { self.root_repo_is_linked_worktree }

    pub(crate) fn build_initial_update(
        &self,
        project_id: u64,
        worktree_id: u64,
    ) -> proto::UpdateWorktree {
        let mut updated_entries = self
            .entries_by_path
            .iter()
            .map(proto::Entry::from)
            .collect::<Vec<_>>();
        updated_entries.sort_unstable_by_key(|e| e.id);

        proto::UpdateWorktree {
            project_id,
            worktree_id,
            abs_path: self.abs_path().to_string_lossy().into_owned(),
            root_name: self.root_name().as_unix_str().to_owned(),
            root_repo_common_dir: self
                .root_repo_common_dir()
                .map(|p| p.to_string_lossy().into_owned()),
            root_repo_is_linked_worktree: self.root_repo_is_linked_worktree,
            updated_entries,
            removed_entries: Vec::new(),
            scan_id: self.scan_id as u64,
            is_last_update: self.completed_scan_id == self.scan_id,
            // Sent in separate messages.
            updated_repositories: Vec::new(),
            removed_repositories: Vec::new(),
        }
    }

    pub fn work_directory_abs_path(&self, work_directory: &WorkDirectory) -> PathBuf {
        match work_directory {
            WorkDirectory::InProject { relative_path } => self.absolutize(relative_path),
            WorkDirectory::AboveProject { absolute_path, .. } => absolute_path.as_ref().to_owned(),
        }
    }

    pub fn absolutize(&self, path: &RelPath) -> PathBuf {
        if path.file_name().is_some() {
            let mut abs_path = self.abs_path.to_string();
            for component in path.components() {
                if !abs_path.ends_with(self.path_style.primary_separator()) {
                    abs_path.push_str(self.path_style.primary_separator());
                }
                abs_path.push_str(component);
            }
            PathBuf::from(abs_path)
        } else {
            self.abs_path.as_path().to_path_buf()
        }
    }

    pub fn contains_entry(&self, entry_id: ProjectEntryId) -> bool {
        self.entries_by_id.get(&entry_id, ()).is_some()
    }

    fn insert_entry(
        &mut self,
        entry: proto::Entry,
        always_included_paths: &PathMatcher,
    ) -> Result<Entry> {
        let entry = Entry::try_from((&self.root_char_bag, always_included_paths, entry))?;
        let old_entry = self.entries_by_id.insert_or_replace(
            PathEntry {
                id: entry.id,
                path: entry.path.clone(),
                is_ignored: entry.is_ignored,
                scan_id: 0,
            },
            (),
        );
        if let Some(old_entry) = old_entry {
            self.entries_by_path.remove(&PathKey(old_entry.path), ());
        }
        self.entries_by_path.insert_or_replace(entry.clone(), ());
        Ok(entry)
    }

    fn delete_entry(&mut self, entry_id: ProjectEntryId) -> Option<Arc<RelPath>> {
        let removed_entry = self.entries_by_id.remove(&entry_id, ())?;
        self.entries_by_path = {
            let mut cursor = self.entries_by_path.cursor::<TraversalProgress>(());
            let mut new_entries_by_path =
                cursor.slice(&TraversalTarget::path(&removed_entry.path), Bias::Left);
            while let Some(entry) = cursor.item() {
                if entry.path.starts_with(&removed_entry.path) {
                    self.entries_by_id.remove(&entry.id, ());
                    cursor.next();
                } else {
                    break;
                }
            }
            new_entries_by_path.append(cursor.suffix(), ());
            new_entries_by_path
        };

        Some(removed_entry.path)
    }

    fn update_abs_path(&mut self, abs_path: Arc<SanitizedPath>, root_name: Arc<RelPath>) {
        self.abs_path = abs_path;
        if root_name != self.root_name {
            self.root_char_bag = root_name
                .as_unix_str()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect();
            self.root_name = root_name;
        }
    }

    pub fn apply_remote_update(
        &mut self,
        update: proto::UpdateWorktree,
        always_included_paths: &PathMatcher,
    ) {
        log::debug!(
            "applying remote worktree update. {} entries updated, {} removed",
            update.updated_entries.len(),
            update.removed_entries.len()
        );
        if let Some(root_name) = RelPath::from_unix_str(&update.root_name).log_err() {
            self.update_abs_path(
                SanitizedPath::new_arc(&Path::new(&update.abs_path)),
                root_name.into(),
            );
        }

        let mut entries_by_path_edits = Vec::new();
        let mut entries_by_id_edits = Vec::new();

        for entry_id in update.removed_entries {
            let entry_id = ProjectEntryId::from_proto(entry_id);
            entries_by_id_edits.push(Edit::Remove(entry_id));
            if let Some(entry) = self.entry_for_id(entry_id) {
                entries_by_path_edits.push(Edit::Remove(PathKey(entry.path.clone())));
            }
        }

        for entry in update.updated_entries {
            let Some(entry) =
                Entry::try_from((&self.root_char_bag, always_included_paths, entry)).log_err()
            else {
                continue;
            };
            if let Some(PathEntry { path, .. }) = self.entries_by_id.get(&entry.id, ()) {
                entries_by_path_edits.push(Edit::Remove(PathKey(path.clone())));
            }
            if let Some(old_entry) = self.entries_by_path.get(&PathKey(entry.path.clone()), ())
                && old_entry.id != entry.id
            {
                entries_by_id_edits.push(Edit::Remove(old_entry.id));
            }
            entries_by_id_edits.push(Edit::Insert(PathEntry {
                id: entry.id,
                path: entry.path.clone(),
                is_ignored: entry.is_ignored,
                scan_id: 0,
            }));
            entries_by_path_edits.push(Edit::Insert(entry));
        }

        self.entries_by_path.edit(entries_by_path_edits, ());
        self.entries_by_id.edit(entries_by_id_edits, ());

        // A `None` from a completed scan is a real repo removal, whereas a `None`
        // mid-scan may just mean the sender hasn't registered the root repo yet.
        match update
            .root_repo_common_dir
            .map(|p| SanitizedPath::new_arc(Path::new(&p)))
        {
            Some(dir) => {
                self.root_repo_common_dir = Some(dir);
                self.root_repo_is_linked_worktree = update.root_repo_is_linked_worktree;
            }
            None if update.is_last_update => {
                self.root_repo_common_dir = None;
                self.root_repo_is_linked_worktree = false;
            }
            None => {}
        }

        self.scan_id = update.scan_id as usize;
        if update.is_last_update {
            self.completed_scan_id = update.scan_id as usize;
        }
    }

    pub fn entry_count(&self) -> usize { self.entries_by_path.summary().count }

    pub fn visible_entry_count(&self) -> usize { self.entries_by_path.summary().non_ignored_count }

    pub fn dir_count(&self) -> usize {
        let summary = self.entries_by_path.summary();
        summary.count - summary.file_count
    }

    pub fn visible_dir_count(&self) -> usize {
        let summary = self.entries_by_path.summary();
        summary.non_ignored_count - summary.non_ignored_file_count
    }

    pub fn file_count(&self) -> usize { self.entries_by_path.summary().file_count }

    pub fn visible_file_count(&self) -> usize {
        self.entries_by_path.summary().non_ignored_file_count
    }

    pub fn deferred_scan_dir_count(&self) -> usize {
        self.entries_by_path.summary().deferred_scan_dir_count
    }

    fn traverse_from_offset(
        &self,
        include_files: bool,
        include_dirs: bool,
        include_ignored: bool,
        start_offset: usize,
    ) -> Traversal<'_> {
        let mut cursor = self.entries_by_path.cursor(());
        cursor.seek(
            &TraversalTarget::Count {
                count: start_offset,
                include_files,
                include_dirs,
                include_ignored,
            },
            Bias::Right,
        );
        Traversal {
            snapshot: self,
            cursor,
            include_files,
            include_dirs,
            include_ignored,
        }
    }

    pub fn traverse_from_path(
        &self,
        include_files: bool,
        include_dirs: bool,
        include_ignored: bool,
        path: &RelPath,
    ) -> Traversal<'_> {
        Traversal::new(self, include_files, include_dirs, include_ignored, path)
    }

    pub fn files(&self, include_ignored: bool, start: usize) -> Traversal<'_> {
        self.traverse_from_offset(true, false, include_ignored, start)
    }

    pub fn directories(&self, include_ignored: bool, start: usize) -> Traversal<'_> {
        self.traverse_from_offset(false, true, include_ignored, start)
    }

    pub fn entries(&self, include_ignored: bool, start: usize) -> Traversal<'_> {
        self.traverse_from_offset(true, true, include_ignored, start)
    }

    pub fn paths(&self) -> impl Iterator<Item = &RelPath> {
        self.entries_by_path
            .cursor::<()>(())
            .filter(move |entry| !entry.path.is_empty())
            .map(|entry| entry.path.as_ref())
    }

    pub fn child_entries<'a>(&'a self, parent_path: &'a RelPath) -> ChildEntriesIter<'a> {
        let options = ChildEntriesOptions {
            include_files: true,
            include_dirs: true,
            include_ignored: true,
        };
        self.child_entries_with_options(parent_path, options)
    }

    pub fn child_entries_with_options<'a>(
        &'a self,
        parent_path: &'a RelPath,
        options: ChildEntriesOptions,
    ) -> ChildEntriesIter<'a> {
        let mut cursor = self.entries_by_path.cursor(());
        cursor.seek(&TraversalTarget::path(parent_path), Bias::Right);
        let traversal = Traversal {
            snapshot: self,
            cursor,
            include_files: options.include_files,
            include_dirs: options.include_dirs,
            include_ignored: options.include_ignored,
        };
        ChildEntriesIter {
            traversal,
            parent_path,
        }
    }

    pub fn root_entry(&self) -> Option<&Entry> { self.entries_by_path.first() }

    /// Returns `None` for a single file worktree, or `Some(self.abs_path())` if
    /// it is a directory.
    pub fn root_dir(&self) -> Option<Arc<Path>> {
        self.root_entry()
            .filter(|entry| entry.is_dir())
            .map(|_| self.abs_path().clone())
    }

    pub fn root_name(&self) -> &RelPath { &self.root_name }

    pub fn root_name_str(&self) -> &str { self.root_name.as_unix_str() }

    pub fn scan_id(&self) -> usize { self.scan_id }

    pub fn entry_for_path(&self, path: &RelPath) -> Option<&Entry> {
        let entry = self.traverse_from_path(true, true, true, path).entry();
        entry.filter(|&entry| entry.path.as_ref() == path)
    }

    /// Whether `path` is gitignored, or lies inside a gitignored directory.
    ///
    /// The contents of ignored directories aren't scanned until explicitly
    /// expanded, so when `path` has no entry this falls back to the ignore
    /// status of its nearest scanned ancestor.
    pub fn is_path_ignored(&self, path: &RelPath) -> bool {
        path.ancestors()
            .find_map(|ancestor| self.entry_for_path(ancestor))
            .is_some_and(|entry| entry.is_ignored)
    }

    /// Resolves a path to an executable using the following heuristics:
    ///
    /// 1. If the path starts with `~`, it is expanded to the user's home directory.
    /// 2. If the path is relative and contains more than one component,
    ///    it is joined to the worktree root path.
    /// 3. If the path is relative and exists in the worktree
    ///    (even if falls under an exclusion filter),
    ///    it is joined to the worktree root path.
    /// 4. Otherwise the path is returned unmodified.
    ///
    /// Relative paths that do not exist in the worktree may
    /// still be found using the `PATH` environment variable.
    pub fn resolve_relative_path(&self, path: PathBuf) -> PathBuf {
        if let Some(path_str) = path.to_str() {
            if let Some(remaining_path) = path_str.strip_prefix("~/") {
                return home_dir().join(remaining_path);
            } else if path_str == "~" {
                return home_dir().to_path_buf();
            }
        }

        if let Ok(rel_path) = RelPath::new(&path, self.path_style)
            && (path.components().count() > 1 || self.entry_for_path(&rel_path).is_some())
        {
            self.abs_path().join(path)
        } else {
            path
        }
    }

    pub fn entry_for_id(&self, id: ProjectEntryId) -> Option<&Entry> {
        let entry = self.entries_by_id.get(&id, ())?;
        self.entry_for_path(&entry.path)
    }

    pub fn path_style(&self) -> PathStyle { self.path_style }
}
impl fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct EntriesById<'a>(&'a SumTree<PathEntry>);
        struct EntriesByPath<'a>(&'a SumTree<Entry>);

        impl fmt::Debug for EntriesByPath<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_map()
                    .entries(self.0.iter().map(|entry| (&entry.path, entry.id)))
                    .finish()
            }
        }

        impl fmt::Debug for EntriesById<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.iter()).finish()
            }
        }

        f.debug_struct("Snapshot")
            .field("id", &self.id)
            .field("root_name", &self.root_name)
            .field("entries_by_path", &EntriesByPath(&self.entries_by_path))
            .field("entries_by_id", &EntriesById(&self.entries_by_id))
            .finish()
    }
}
