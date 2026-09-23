use std::sync::Arc;

use aa_gpui_fuzzy::{
    PathMatchCandidate, PathMatchCandidateSet as FuzzyPathMatchCandidateSet, PathStyle,
};
use util::rel_path::RelPath;
use worktree::{Snapshot, Traversal};

pub struct PathMatchCandidateSet {
    pub snapshot: Snapshot,
    pub include_ignored: bool,
    pub include_root_name: bool,
    pub candidates: Candidates,
}

pub enum Candidates {
    /// Only consider directories.
    Directories,
    /// Only consider files.
    Files,
    /// Consider directories and files.
    Entries,
}

impl<'a> FuzzyPathMatchCandidateSet<'a> for PathMatchCandidateSet {
    type Candidates = PathMatchCandidateSetIter<'a>;

    fn id(&self) -> usize {
        self.snapshot.id().to_usize()
    }

    fn len(&self) -> usize {
        match self.candidates {
            Candidates::Files => {
                if self.include_ignored {
                    self.snapshot.file_count()
                } else {
                    self.snapshot.visible_file_count()
                }
            }

            Candidates::Directories => {
                if self.include_ignored {
                    self.snapshot.dir_count()
                } else {
                    self.snapshot.visible_dir_count()
                }
            }

            Candidates::Entries => {
                if self.include_ignored {
                    self.snapshot.entry_count()
                } else {
                    self.snapshot.visible_entry_count()
                }
            }
        }
    }

    fn prefix(&self) -> Arc<RelPath> {
        if self.snapshot.root_entry().is_some_and(|e| e.is_file()) || self.include_root_name {
            self.snapshot.root_name().into()
        } else {
            RelPath::empty_arc()
        }
    }

    fn root_is_file(&self) -> bool {
        self.snapshot.root_entry().is_some_and(|f| f.is_file())
    }

    fn path_style(&self) -> PathStyle {
        self.snapshot.path_style()
    }

    fn candidates(&'a self, start: usize) -> Self::Candidates {
        PathMatchCandidateSetIter {
            traversal: match self.candidates {
                Candidates::Directories => self.snapshot.directories(self.include_ignored, start),
                Candidates::Files => self.snapshot.files(self.include_ignored, start),
                Candidates::Entries => self.snapshot.entries(self.include_ignored, start),
            },
        }
    }
}

pub struct PathMatchCandidateSetIter<'a> {
    traversal: Traversal<'a>,
}

impl<'a> Iterator for PathMatchCandidateSetIter<'a> {
    type Item = PathMatchCandidate<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.traversal
            .next()
            .map(|entry| PathMatchCandidate {
                is_dir: entry.kind.is_dir(),
                path: &entry.path,
                char_bag: entry.char_bag,
            })
    }
}
