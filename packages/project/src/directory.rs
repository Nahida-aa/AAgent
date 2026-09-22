use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::Fs;
use gpui::{App, Entity, Task};
use util::paths::PathStyle;

use crate::Project;

pub struct DirectoryItem {
    pub path: PathBuf,
    pub is_dir: bool,
}

pub enum DirectoryLister {
    Project(Entity<Project>),
    Local(Entity<Project>, Arc<dyn Fs>),
}

impl std::fmt::Debug for DirectoryLister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectoryLister::Project(project) => {
                write!(f, "DirectoryLister::Project({project:?})")
            }
            DirectoryLister::Local(project, _) => {
                write!(f, "DirectoryLister::Local({project:?})")
            }
        }
    }
}


impl DirectoryLister {
    pub fn is_local(&self, cx: &App) -> bool {
        match self {
            DirectoryLister::Local(..) => true,
            DirectoryLister::Project(project) => project.read(cx).is_local(),
        }
    }

    pub fn resolve_tilde<'a>(&self, path: &'a String, cx: &App) -> Cow<'a, str> {
        if self.is_local(cx) {
            shellexpand::tilde(path)
        } else {
            Cow::from(path)
        }
    }

    pub fn default_query(&self, cx: &mut App) -> String {
        let project = match self {
            DirectoryLister::Project(project) => project,
            DirectoryLister::Local(project, _) => project,
        }
        .read(cx);
        let path_style = project.path_style(cx);
        project
            .visible_worktrees(cx)
            .next()
            .map(|worktree| worktree.read(cx).abs_path().to_string_lossy().into_owned())
            .or_else(|| std::env::home_dir().map(|dir| dir.to_string_lossy().into_owned()))
            .map(|mut s| {
                s.push_str(path_style.primary_separator());
                s
            })
            .unwrap_or_else(|| {
                if path_style.is_windows() {
                    "C:\\"
                } else {
                    "~/"
                }
                .to_string()
            })
    }

    pub fn list_directory(&self, path: String, cx: &mut App) -> Task<Result<Vec<DirectoryItem>>> {
        match self {
            DirectoryLister::Project(project) => {
                project.update(cx, |project, cx| project.list_directory(path, cx))
            }
            DirectoryLister::Local(_, fs) => {
                let fs = fs.clone();
                cx.background_spawn(async move {
                    let mut results = vec![];
                    let expanded = shellexpand::tilde(&path);
                    let query = Path::new(expanded.as_ref());
                    let mut response = fs.read_dir(query).await?;
                    while let Some(path) = response.next().await {
                        let path = path?;
                        if let Some(file_name) = path.file_name() {
                            results.push(DirectoryItem {
                                path: PathBuf::from(file_name.to_os_string()),
                                is_dir: fs.is_dir(&path).await,
                            });
                        }
                    }
                    Ok(results)
                })
            }
        }
    }

    pub fn path_style(&self, cx: &App) -> PathStyle {
        match self {
            Self::Local(project, ..) | Self::Project(project, ..) => {
                project.read(cx).path_style(cx)
            }
        }
    }
}
