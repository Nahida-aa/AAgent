use std::path::{Path, PathBuf};

use gpui::{App, SharedString};
use remote::{RemoteConnectionOptions, same_remote_connection_identity};
use util::path_list::PathList;

use crate::Project;
use crate::worktree_store::WorktreePaths;

/// Identifies a project group by a set of paths the workspaces in this group
/// have.
///
/// Paths are mapped to their main worktree path first so we can group
/// workspaces by main repos.
#[derive(PartialEq, Eq, Hash, Clone, Debug, Default)]
pub struct ProjectGroupKey {
    /// The paths of the main worktrees for this project group.
    paths: PathList,
    host: Option<RemoteConnectionOptions>,
}

impl ProjectGroupKey {
    /// Creates a new `ProjectGroupKey` with the given path list.
    ///
    /// The path list should point to the git main worktree paths for a project.
    pub fn new(host: Option<RemoteConnectionOptions>, paths: PathList) -> Self {
        Self { paths, host }
    }

    pub fn from_project(project: &Project, cx: &App) -> Self {
        let paths = project.worktree_paths(cx);
        let host = project.remote_connection_options(cx);
        Self {
            paths: paths.main_worktree_path_list().clone(),
            host,
        }
    }

    pub fn from_worktree_paths(
        paths: &WorktreePaths,
        host: Option<RemoteConnectionOptions>,
    ) -> Self {
        Self {
            paths: paths.main_worktree_path_list().clone(),
            host,
        }
    }

    pub fn path_list(&self) -> &PathList { &self.paths }

    pub fn display_name(
        &self,
        path_detail_map: &std::collections::HashMap<PathBuf, usize>,
    ) -> SharedString {
        let mut names = Vec::with_capacity(self.paths.paths().len());
        for abs_path in self.paths.ordered_paths() {
            let detail = path_detail_map.get(abs_path).copied().unwrap_or(0);
            // Strip a `.git` extension for display (bare clones like `foo.git`
            // should display as `foo`, matching the titlebar).
            let display_path = if abs_path.extension() == Some(std::ffi::OsStr::new("git")) {
                std::borrow::Cow::Owned(abs_path.with_extension(""))
            } else {
                std::borrow::Cow::Borrowed(abs_path.as_path())
            };
            let suffix = path_suffix(&display_path, detail);
            if !suffix.is_empty() {
                names.push(suffix);
            }
        }
        if names.is_empty() {
            "Empty Workspace".into()
        } else {
            names.join(", ").into()
        }
    }

    pub fn host(&self) -> Option<RemoteConnectionOptions> { self.host.clone() }

    pub fn matches(&self, other: &ProjectGroupKey) -> bool {
        self.paths == other.paths
            && same_remote_connection_identity(self.host.as_ref(), other.host.as_ref())
    }
}

pub fn path_suffix(path: &Path, detail: usize) -> String {
    let mut components: Vec<_> = path
        .components()
        .rev()
        .filter_map(|component| match component {
            std::path::Component::Normal(s) => Some(s.to_string_lossy()),
            _ => None,
        })
        .take(detail + 1)
        .collect();
    components.reverse();
    components.join("/")
}

impl Project {
    pub fn project_group_key(&self, cx: &App) -> ProjectGroupKey {
        ProjectGroupKey::from_project(self, cx)
    }
}
