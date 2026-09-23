use super::*;

use super::Project;
use crate::ProjectPath;
use crate::directory::{DirectoryItem, DirectoryLister};
use crate::path::ResolvedPath;
use ::rpc::proto::{self, REMOTE_SERVER_PROJECT_ID};
use anyhow::{Context as _, Result, anyhow};
use futures::{
    StreamExt,
    channel::mpsc::{self, UnboundedReceiver},
    future::try_join_all,
};
use gpui::{
    App, AppContext, AsyncApp, BorrowAppContext, Context, Entity, EventEmitter, Hsla, SharedString,
    Task, TaskExt, WeakEntity, Window,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use util::ResultExt;
use util::paths::{PathStyle, SanitizedPath};
use util::rel_path::RelPath;
use worktree::Worktree;

impl Project {
    pub fn find_project_path(&self, path: impl AsRef<Path>, cx: &App) -> Option<ProjectPath> {
        let path_style = self.path_style(cx);
        let path = path.as_ref();
        let worktree_store = self.worktree_store.read(cx);

        if util::paths::is_absolute(&path.to_string_lossy(), path_style) {
            for worktree in worktree_store.visible_worktrees(cx) {
                let worktree_abs_path = worktree.read(cx).abs_path();

                if let Ok(relative_path) = path.strip_prefix(worktree_abs_path)
                    && let Ok(path) = RelPath::new(relative_path, path_style)
                {
                    return Some(ProjectPath {
                        worktree_id: worktree.read(cx).id(),
                        path: path.into_arc(),
                    });
                }
            }
        } else {
            // First pass: for each worktree, try two interpretations of the path and
            // return whichever finds an existing entry first:
            //   (a) Strip the worktree root name as a prefix.
            //   (b) Treat the path as a literal worktree-relative path.
            for worktree in worktree_store.visible_worktrees(cx) {
                let worktree = worktree.read(cx);
                if let Ok(relative_path) = path.strip_prefix(worktree.root_name().as_std_path())
                    && let Ok(rel_path) = RelPath::new(relative_path, path_style)
                    && let Some(entry) = worktree.entry_for_path(&rel_path)
                {
                    return Some(ProjectPath {
                        worktree_id: worktree.id(),
                        path: entry.path.clone(),
                    });
                }
                if let Ok(rel_path) = RelPath::new(path, path_style)
                    && let Some(entry) = worktree.entry_for_path(&rel_path)
                {
                    return Some(ProjectPath {
                        worktree_id: worktree.id(),
                        path: entry.path.clone(),
                    });
                }
            }

            // Second pass: strip the worktree root name prefix without requiring the
            // entry to exist, to allow resolving paths that don't exist yet.
            for worktree in worktree_store.visible_worktrees(cx) {
                let worktree_root_name = worktree.read(cx).root_name();
                if let Ok(relative_path) = path.strip_prefix(worktree_root_name.as_std_path())
                    && let Ok(path) = RelPath::new(relative_path, path_style)
                {
                    return Some(ProjectPath {
                        worktree_id: worktree.read(cx).id(),
                        path: path.into_arc(),
                    });
                }
            }
        }

        None
    }

    /// If there's only one visible worktree, returns the given worktree-relative path with no prefix.
    ///
    /// Otherwise, returns the full path for the project path (obtained by prefixing the worktree-relative path with the name of the worktree).
    pub fn short_full_path_for_project_path(
        &self,
        project_path: &ProjectPath,
        cx: &App,
    ) -> Option<String> {
        let path_style = self.path_style(cx);
        if self.visible_worktrees(cx).take(2).count() < 2 {
            return Some(project_path.path.display(path_style).to_string());
        }
        self.worktree_for_id(project_path.worktree_id, cx)
            .map(|worktree| {
                let worktree_name = worktree.read(cx).root_name();
                worktree_name
                    .join(&project_path.path)
                    .display(path_style)
                    .to_string()
            })
    }
    pub fn project_path_for_absolute_path(&self, abs_path: &Path, cx: &App) -> Option<ProjectPath> {
        self.worktree_store
            .read(cx)
            .project_path_for_absolute_path(abs_path, cx)
    }
    pub fn get_workspace_root(&self, project_path: &ProjectPath, cx: &App) -> Option<PathBuf> {
        Some(
            self.worktree_for_id(project_path.worktree_id, cx)?
                .read(cx)
                .abs_path()
                .to_path_buf(),
        )
    }
    pub fn visibility_for_paths(
        &self,
        paths: &[PathBuf],
        exclude_sub_dirs: bool,
        cx: &App,
    ) -> Option<bool> {
        paths
            .iter()
            .map(|path| self.visibility_for_path(path, exclude_sub_dirs, cx))
            .max()
            .flatten()
    }
    pub fn visibility_for_path(
        &self,
        path: &Path,
        exclude_sub_dirs: bool,
        cx: &App,
    ) -> Option<bool> {
        let path = SanitizedPath::new(path).as_path();
        let path_style = self.path_style(cx);
        self.worktrees(cx)
            .filter_map(|worktree| {
                let worktree = worktree.read(cx);
                let abs_path = worktree.abs_path();
                let relative_path = path_style.strip_prefix(path, abs_path.as_ref())?;
                // Don't exclude the worktree root itself, only actual subdirectories
                let is_subpath = !relative_path.is_empty();
                // Gitignored subtrees aren't scanned, so their contents don't
                // meaningfully belong to this project (e.g. nested checkouts
                // in an ignored directory). Treat such paths as not contained
                // so opening them behaves like opening an unrelated path.
                if is_subpath && worktree.is_path_ignored(&relative_path) {
                    return None;
                }
                let is_dir = worktree
                    .entry_for_path(&relative_path)
                    .is_some_and(|e| e.is_dir());
                let contains = !exclude_sub_dirs || !is_dir || !is_subpath;
                contains.then(|| worktree.is_visible())
            })
            .max()
    }
    pub fn visibility_for_subpaths(&self, paths: &[PathBuf], cx: &App) -> Option<bool> {
        paths
            .iter()
            .map(|path| self.visibility_for_subpath(path, cx))
            .max()
            .flatten()
    }
    fn visibility_for_subpath(&self, path: &Path, cx: &App) -> Option<bool> {
        let path = SanitizedPath::new(path).as_path();
        let path_style = self.path_style(cx);
        self.worktrees(cx)
            .filter_map(|worktree| {
                let worktree = worktree.read(cx);
                let abs_path = worktree.abs_path();
                let relative_path = path_style.strip_prefix(path, abs_path.as_ref())?;
                let is_subpath =
                    !relative_path.is_empty() && !worktree.is_path_ignored(&relative_path);
                is_subpath.then(|| worktree.is_visible())
            })
            .max()
    }
    pub fn resolve_path_in_buffer(
        &self,
        path: &str,
        buffer: &Entity<Buffer>,
        cx: &mut Context<Self>,
    ) -> Task<Option<ResolvedPath>> {
        if util::paths::is_absolute(path, self.path_style(cx)) || path.starts_with("~") {
            self.resolve_abs_path(path, cx)
        } else {
            self.resolve_path_in_worktrees(path, buffer, cx)
        }
    }
    pub fn resolve_abs_file_path(
        &self,
        path: &str,
        cx: &mut Context<Self>,
    ) -> Task<Option<ResolvedPath>> {
        let resolve_task = self.resolve_abs_path(path, cx);
        cx.background_spawn(async move {
            let resolved_path = resolve_task.await;
            resolved_path.filter(|path| path.is_file())
        })
    }
    pub fn resolve_abs_path(&self, path: &str, cx: &App) -> Task<Option<ResolvedPath>> {
        if self.is_local() {
            let expanded = PathBuf::from(shellexpand::tilde(&path).into_owned());
            let fs = self.fs.clone();
            cx.background_spawn(async move {
                let metadata = fs.metadata(&expanded).await.ok().flatten();

                metadata.map(|metadata| ResolvedPath::AbsPath {
                    path: expanded.to_string_lossy().into_owned(),
                    is_dir: metadata.is_dir,
                })
            })
        } else if let Some(ssh_client) = self.remote_client.as_ref() {
            let request =
                ssh_client
                    .read(cx)
                    .proto_client()
                    .request(::rpc::proto::GetPathMetadata {
                        project_id: REMOTE_SERVER_PROJECT_ID,
                        path: path.into(),
                    });
            cx.background_spawn(async move {
                let response = request.await.log_err()?;
                if response.exists {
                    Some(ResolvedPath::AbsPath {
                        path: response.path,
                        is_dir: response.is_dir,
                    })
                } else {
                    None
                }
            })
        } else {
            Task::ready(None)
        }
    }
    fn resolve_path_in_worktrees(
        &self,
        path: &str,
        buffer: &Entity<Buffer>,
        cx: &mut Context<Self>,
    ) -> Task<Option<ResolvedPath>> {
        let mut candidates = vec![];
        let path_style = self.path_style(cx);
        if let Ok(path) = RelPath::new(path.as_ref(), path_style) {
            candidates.push(path.into_arc());
        }

        if let Some(file) = buffer.read(cx).file()
            && let Some(dir) = file.path().parent()
        {
            if let Some(joined) = path_style.join(&*dir.display(path_style), path)
                && let Some(joined) = RelPath::new(joined.as_ref(), path_style).ok()
            {
                candidates.push(joined.into_arc());
            }
        }

        let buffer_worktree_id = buffer.read(cx).file().map(|file| file.worktree_id(cx));
        let worktrees_with_ids: Vec<_> = self
            .worktrees(cx)
            .map(|worktree| {
                let id = worktree.read(cx).id();
                (worktree, id)
            })
            .collect();

        cx.spawn(async move |_, cx| {
            if let Some(buffer_worktree_id) = buffer_worktree_id
                && let Some((worktree, _)) = worktrees_with_ids
                    .iter()
                    .find(|(_, id)| *id == buffer_worktree_id)
            {
                for candidate in candidates.iter() {
                    if let Some(path) = Self::resolve_path_in_worktree(worktree, candidate, cx) {
                        return Some(path);
                    }
                }
            }
            for (worktree, id) in worktrees_with_ids {
                if Some(id) == buffer_worktree_id {
                    continue;
                }
                for candidate in candidates.iter() {
                    if let Some(path) = Self::resolve_path_in_worktree(&worktree, candidate, cx) {
                        return Some(path);
                    }
                }
            }
            None
        })
    }
    fn resolve_path_in_worktree(
        worktree: &Entity<Worktree>,
        path: &RelPath,
        cx: &mut AsyncApp,
    ) -> Option<ResolvedPath> {
        worktree.read_with(cx, |worktree, _| {
            worktree.entry_for_path(path).map(|entry| {
                let project_path = ProjectPath {
                    worktree_id: worktree.id(),
                    path: entry.path.clone(),
                };
                ResolvedPath::ProjectPath {
                    project_path,
                    is_dir: entry.is_dir(),
                }
            })
        })
    }
    pub fn try_windows_path_to_wsl(
        &self,
        abs_path: &Path,
        cx: &App,
    ) -> impl Future<Output = Result<PathBuf>> + use<> {
        let fut = if cfg!(windows)
            && let (
                ProjectClientState::Local | ProjectClientState::Shared { .. },
                Some(remote_client),
            ) = (&self.client_state, &self.remote_client)
            && let RemoteConnectionOptions::Wsl(wsl) = remote_client.read(cx).connection_options()
        {
            Either::Left(wsl.abs_windows_path_to_wsl_path(abs_path))
        } else {
            Either::Right(abs_path.to_owned())
        };
        async move {
            match fut {
                Either::Left(fut) => fut.await.map(Into::into),
                Either::Right(path) => Ok(path),
            }
        }
    }
    pub fn list_directory(
        &self,
        query: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<DirectoryItem>>> {
        if self.is_local() {
            DirectoryLister::Local(cx.entity(), self.fs.clone()).list_directory(query, cx)
        } else if let Some(session) = self.remote_client.as_ref() {
            let request = proto::ListRemoteDirectory {
                dev_server_id: REMOTE_SERVER_PROJECT_ID,
                path: query,
                config: Some(proto::ListRemoteDirectoryConfig { is_dir: true }),
            };

            let response = session.read(cx).proto_client().request(request);
            cx.background_spawn(async move {
                let proto::ListRemoteDirectoryResponse {
                    entries,
                    entry_info,
                } = response.await?;
                Ok(entries
                    .into_iter()
                    .zip(entry_info)
                    .map(|(entry, info)| DirectoryItem {
                        path: PathBuf::from(entry),
                        is_dir: info.is_dir,
                    })
                    .collect())
            })
        } else {
            Task::ready(Err(anyhow!("cannot list directory in remote project")))
        }
    }
}
