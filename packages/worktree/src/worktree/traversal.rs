use super::*;

#[derive(Debug)]
pub struct Traversal<'a> {
    pub(crate) snapshot: &'a Snapshot,
    pub(crate) cursor: sum_tree::Cursor<'a, 'static, Entry, TraversalProgress<'a>>,
    pub(crate) include_ignored: bool,
    pub(crate) include_files: bool,
    pub(crate) include_dirs: bool,
}

impl<'a> Traversal<'a> {
    pub(crate) fn new(
        snapshot: &'a Snapshot,
        include_files: bool,
        include_dirs: bool,
        include_ignored: bool,
        start_path: &RelPath,
    ) -> Self {
        let mut cursor = snapshot.entries_by_path.cursor(());
        cursor.seek(&TraversalTarget::path(start_path), Bias::Left);
        let mut traversal = Self {
            snapshot,
            cursor,
            include_files,
            include_dirs,
            include_ignored,
        };
        if traversal.end_offset() == traversal.start_offset() {
            traversal.next();
        }
        traversal
    }

    pub fn advance(&mut self) -> bool { self.advance_by(1) }

    pub fn advance_by(&mut self, count: usize) -> bool {
        self.cursor.seek_forward(
            &TraversalTarget::Count {
                count: self.end_offset() + count,
                include_dirs: self.include_dirs,
                include_files: self.include_files,
                include_ignored: self.include_ignored,
            },
            Bias::Left,
        )
    }

    pub fn advance_to_sibling(&mut self) -> bool {
        while let Some(entry) = self.cursor.item() {
            self.cursor
                .seek_forward(&TraversalTarget::successor(&entry.path), Bias::Left);
            if let Some(entry) = self.cursor.item()
                && (self.include_files || !entry.is_file())
                && (self.include_dirs || !entry.is_dir())
                && (self.include_ignored || !entry.is_ignored || entry.is_always_included)
            {
                return true;
            }
        }
        false
    }

    pub fn back_to_parent(&mut self) -> bool {
        let Some(parent_path) = self.cursor.item().and_then(|entry| entry.path.parent()) else {
            return false;
        };
        self.cursor
            .seek(&TraversalTarget::path(parent_path), Bias::Left)
    }

    pub fn entry(&self) -> Option<&'a Entry> { self.cursor.item() }

    pub fn snapshot(&self) -> &'a Snapshot { self.snapshot }

    pub fn start_offset(&self) -> usize {
        self.cursor
            .start()
            .count(self.include_files, self.include_dirs, self.include_ignored)
    }

    pub fn end_offset(&self) -> usize {
        self.cursor
            .end()
            .count(self.include_files, self.include_dirs, self.include_ignored)
    }
}

impl<'a> Iterator for Traversal<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.entry() {
            self.advance();
            Some(item)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PathTarget<'a> {
    Path(&'a RelPath),
    Successor(&'a RelPath),
}
impl PathTarget<'_> {
    fn cmp_path(&self, other: &RelPath) -> Ordering {
        match self {
            PathTarget::Path(path) => path.cmp(&other),
            PathTarget::Successor(path) => {
                if other.starts_with(path) {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
        }
    }
}

impl<'a, S: Summary> SeekTarget<'a, PathSummary<S>, PathProgress<'a>> for PathTarget<'_> {
    fn cmp(&self, cursor_location: &PathProgress<'a>, _: S::Context<'_>) -> Ordering {
        self.cmp_path(cursor_location.max_path)
    }
}

impl<'a, S: Summary> SeekTarget<'a, PathSummary<S>, TraversalProgress<'a>> for PathTarget<'_> {
    fn cmp(&self, cursor_location: &TraversalProgress<'a>, _: S::Context<'_>) -> Ordering {
        self.cmp_path(cursor_location.max_path)
    }
}

#[derive(Debug)]
pub(crate) enum TraversalTarget<'a> {
    Path(PathTarget<'a>),
    Count {
        count: usize,
        include_files: bool,
        include_ignored: bool,
        include_dirs: bool,
    },
}
impl<'a> TraversalTarget<'a> {
    fn path(path: &'a RelPath) -> Self { Self::Path(PathTarget::Path(path)) }

    fn successor(path: &'a RelPath) -> Self { Self::Path(PathTarget::Successor(path)) }

    fn cmp_progress(&self, progress: &TraversalProgress) -> Ordering {
        match self {
            TraversalTarget::Path(path) => path.cmp_path(progress.max_path),
            TraversalTarget::Count {
                count,
                include_files,
                include_dirs,
                include_ignored,
            } => Ord::cmp(
                count,
                &progress.count(*include_files, *include_dirs, *include_ignored),
            ),
        }
    }
}

impl<'a> SeekTarget<'a, EntrySummary, TraversalProgress<'a>> for TraversalTarget<'_> {
    fn cmp(&self, cursor_location: &TraversalProgress<'a>, _: ()) -> Ordering {
        self.cmp_progress(cursor_location)
    }
}

impl<'a> SeekTarget<'a, PathSummary<sum_tree::NoSummary>, TraversalProgress<'a>>
    for TraversalTarget<'_>
{
    fn cmp(&self, cursor_location: &TraversalProgress<'a>, _: ()) -> Ordering {
        self.cmp_progress(cursor_location)
    }
}

pub struct ChildEntriesOptions {
    pub include_files: bool,
    pub include_dirs: bool,
    pub include_ignored: bool,
}

pub struct ChildEntriesIter<'a> {
    pub(crate) parent_path: &'a RelPath,
    pub(crate) traversal: Traversal<'a>,
}

impl<'a> Iterator for ChildEntriesIter<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.traversal.entry()
            && item.path.starts_with(self.parent_path)
        {
            self.traversal.advance_to_sibling();
            return Some(item);
        }
        None
    }
}
