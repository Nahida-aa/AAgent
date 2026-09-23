use std::path::PathBuf;
use std::sync::Arc;

use gpui::App;
use settings::SettingsLocation;
use util::ResultExt;
use util::rel_path::RelPath;
use worktree::WorktreeId;

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ProjectPath {
    pub worktree_id: WorktreeId,
    pub path: Arc<RelPath>,
}

impl ProjectPath {
    pub fn from_file(value: &dyn language::File, cx: &App) -> Self {
        ProjectPath {
            worktree_id: value.worktree_id(cx),
            path: value.path().clone(),
        }
    }

    pub fn from_proto(p: rpc::proto::ProjectPath) -> Option<Self> {
        Some(Self {
            worktree_id: WorktreeId::from_proto(p.worktree_id),
            path: RelPath::from_unix_str(&p.path).log_err()?.into(),
        })
    }

    pub fn to_proto(&self) -> rpc::proto::ProjectPath {
        rpc::proto::ProjectPath {
            worktree_id: self.worktree_id.to_proto(),
            path: self.path.as_ref().as_unix_str().to_owned(),
        }
    }

    pub fn root_path(worktree_id: WorktreeId) -> Self {
        Self {
            worktree_id,
            path: RelPath::empty_arc(),
        }
    }

    pub fn starts_with(&self, other: &ProjectPath) -> bool {
        self.worktree_id == other.worktree_id && self.path.starts_with(&other.path)
    }
}

/// ResolvedPath is a path that has been resolved to either a ProjectPath
/// or an AbsPath and that *exists*.
#[derive(Debug, Clone)]
pub enum ResolvedPath {
    ProjectPath {
        project_path: ProjectPath,
        is_dir: bool,
    },
    AbsPath {
        path: String,
        is_dir: bool,
    },
}
impl ResolvedPath {
    pub fn abs_path(&self) -> Option<&str> {
        match self {
            Self::AbsPath { path, .. } => Some(path),
            _ => None,
        }
    }

    pub fn into_abs_path(self) -> Option<String> {
        match self {
            Self::AbsPath { path, .. } => Some(path),
            _ => None,
        }
    }

    pub fn project_path(&self) -> Option<&ProjectPath> {
        match self {
            Self::ProjectPath { project_path, .. } => Some(project_path),
            _ => None,
        }
    }

    pub fn is_file(&self) -> bool { !self.is_dir() }

    pub fn is_dir(&self) -> bool {
        match self {
            Self::ProjectPath { is_dir, .. } => *is_dir,
            Self::AbsPath { is_dir, .. } => *is_dir,
        }
    }
}

impl<'a> From<&'a ProjectPath> for SettingsLocation<'a> {
    fn from(val: &'a ProjectPath) -> Self {
        SettingsLocation {
            worktree_id: val.worktree_id,
            path: val.path.as_ref(),
        }
    }
}

impl<P: Into<Arc<RelPath>>> From<(WorktreeId, P)> for ProjectPath {
    fn from((worktree_id, path): (WorktreeId, P)) -> Self {
        Self {
            worktree_id,
            path: path.into(),
        }
    }
}
