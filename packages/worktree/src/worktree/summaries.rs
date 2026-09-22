use super::*;

#[derive(Clone, Debug)]
pub struct PathProgress<'a> {
    pub max_path: &'a RelPath,
}

#[derive(Clone, Debug)]
pub struct PathSummary<S> {
    pub max_path: Arc<RelPath>,
    pub item_summary: S,
}
impl<S: Summary> Summary for PathSummary<S> {
    type Context<'a> = S::Context<'a>;

    fn zero(cx: Self::Context<'_>) -> Self {
        Self {
            max_path: RelPath::empty_arc(),
            item_summary: S::zero(cx),
        }
    }

    fn add_summary(&mut self, rhs: &Self, cx: Self::Context<'_>) {
        self.max_path = rhs.max_path.clone();
        self.item_summary.add_summary(&rhs.item_summary, cx);
    }
}

impl<'a, S: Summary> sum_tree::Dimension<'a, PathSummary<S>> for PathProgress<'a> {
    fn zero(_: <PathSummary<S> as Summary>::Context<'_>) -> Self {
        Self {
            max_path: RelPath::empty(),
        }
    }

    fn add_summary(
        &mut self,
        summary: &'a PathSummary<S>,
        _: <PathSummary<S> as Summary>::Context<'_>,
    ) {
        self.max_path = summary.max_path.as_ref()
    }
}
impl<'a, S: Summary> sum_tree::Dimension<'a, PathSummary<S>> for PathKey {
    fn zero(_: S::Context<'_>) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a PathSummary<S>, _: S::Context<'_>) {
        self.0 = summary.max_path.clone();
    }
}

impl<'a, S: Summary> sum_tree::Dimension<'a, PathSummary<S>> for TraversalProgress<'a> {
    fn zero(_cx: S::Context<'_>) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a PathSummary<S>, _: S::Context<'_>) {
        self.max_path = summary.max_path.as_ref();
    }
}
impl<'a> sum_tree::Dimension<'a, PathSummary<GitSummary>> for GitSummary {
    fn zero(_cx: ()) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a PathSummary<GitSummary>, _: ()) {
        *self += summary.item_summary
    }
}

impl<'a>
    sum_tree::SeekTarget<'a, PathSummary<GitSummary>, Dimensions<TraversalProgress<'a>, GitSummary>>
    for PathTarget<'_>
{
    fn cmp(
        &self,
        cursor_location: &Dimensions<TraversalProgress<'a>, GitSummary>,
        _: (),
    ) -> Ordering {
        self.cmp_path(cursor_location.0.max_path)
    }
}
#[derive(Clone, Debug)]
pub struct EntrySummary {
    pub(crate) max_path: Arc<RelPath>,
    pub(crate) count: usize,
    pub(crate) non_ignored_count: usize,
    pub(crate) file_count: usize,
    pub(crate) non_ignored_file_count: usize,
    pub(crate) deferred_scan_dir_count: usize,
}

impl Default for EntrySummary {
    fn default() -> Self {
        Self {
            max_path: Arc::from(RelPath::empty()),
            count: 0,
            non_ignored_count: 0,
            file_count: 0,
            non_ignored_file_count: 0,
            deferred_scan_dir_count: 0,
        }
    }
}

impl sum_tree::ContextLessSummary for EntrySummary {
    fn zero() -> Self { Default::default() }

    fn add_summary(&mut self, rhs: &Self) {
        self.max_path = rhs.max_path.clone();
        self.count += rhs.count;
        self.non_ignored_count += rhs.non_ignored_count;
        self.file_count += rhs.file_count;
        self.non_ignored_file_count += rhs.non_ignored_file_count;
        self.deferred_scan_dir_count += rhs.deferred_scan_dir_count;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PathEntry {
    pub(crate) id: ProjectEntryId,
    pub(crate) path: Arc<RelPath>,
    pub(crate) is_ignored: bool,
    pub(crate) scan_id: usize,
}

impl sum_tree::Item for PathEntry {
    type Summary = PathEntrySummary;

    fn summary(&self, _cx: ()) -> Self::Summary { PathEntrySummary { max_id: self.id } }
}

impl sum_tree::KeyedItem for PathEntry {
    type Key = ProjectEntryId;

    fn key(&self) -> Self::Key { self.id }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PathEntrySummary {
    max_id: ProjectEntryId,
}

impl sum_tree::ContextLessSummary for PathEntrySummary {
    fn zero() -> Self { Default::default() }

    fn add_summary(&mut self, summary: &Self) { self.max_id = summary.max_id; }
}

impl<'a> sum_tree::Dimension<'a, PathEntrySummary> for ProjectEntryId {
    fn zero(_cx: ()) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a PathEntrySummary, _: ()) { *self = summary.max_id; }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PathKey(pub Arc<RelPath>);
impl Default for PathKey {
    fn default() -> Self { Self(RelPath::empty_arc()) }
}

impl<'a> sum_tree::Dimension<'a, EntrySummary> for PathKey {
    fn zero(_cx: ()) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a EntrySummary, _: ()) {
        self.0 = summary.max_path.clone();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TraversalProgress<'a> {
    pub(crate) max_path: &'a RelPath,
    pub(crate) count: usize,
    pub(crate) non_ignored_count: usize,
    pub(crate) file_count: usize,
    pub(crate) non_ignored_file_count: usize,
}

impl TraversalProgress<'_> {
    pub(crate) fn count(&self, include_files: bool, include_dirs: bool, include_ignored: bool) -> usize {
        match (include_files, include_dirs, include_ignored) {
            (true, true, true) => self.count,
            (true, true, false) => self.non_ignored_count,
            (true, false, true) => self.file_count,
            (true, false, false) => self.non_ignored_file_count,
            (false, true, true) => self.count - self.file_count,
            (false, true, false) => self.non_ignored_count - self.non_ignored_file_count,
            (false, false, _) => 0,
        }
    }
}

impl<'a> sum_tree::Dimension<'a, EntrySummary> for TraversalProgress<'a> {
    fn zero(_cx: ()) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a EntrySummary, _: ()) {
        self.max_path = summary.max_path.as_ref();
        self.count += summary.count;
        self.non_ignored_count += summary.non_ignored_count;
        self.file_count += summary.file_count;
        self.non_ignored_file_count += summary.non_ignored_file_count;
    }
}

impl Default for TraversalProgress<'_> {
    fn default() -> Self {
        Self {
            max_path: RelPath::empty(),
            count: 0,
            non_ignored_count: 0,
            file_count: 0,
            non_ignored_file_count: 0,
        }
    }
}
