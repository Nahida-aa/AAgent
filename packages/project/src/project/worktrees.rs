use super::*;

use anyhow::{Context as _, Result, anyhow};
use futures::future::join_all;
use gpui::{App, AppContext as _, Context, Entity, Task, TaskExt as _};
use util::paths::{home_dir, PathStyle};
use util::rel_path::RelPath;
use worktree::{CreatedEntry, Entry, ProjectEntryId, Worktree, WorktreeId};

use super::Project;
use crate::types::*;
use crate::{Event, ProjectPath};
use crate::worktree_store::{WorktreeStoreEvent, WorktreePaths};

impl Project {
    /// All worktrees in this project.
    pub fn worktrees<'a>(
        &self,
        cx: &'a App,
    ) -> impl 'a + DoubleEndedIterator<Item = Entity<Worktree>> {
        self.worktree_store.read(cx).worktrees()
    }

    /// Collect all user-visible worktrees, the ones that appear in the project panel.
    #[inline]
    pub fn visible_worktrees<'a>(
        &'a self,
        cx: &'a App,
    ) -> impl 'a + DoubleEndedIterator<Item = Entity<Worktree>> {
        self.worktree_store.read(cx).visible_worktrees(cx)
    }
    pub(crate) fn default_visible_worktree_paths(
        worktree_store: &WorktreeStore,
        cx: &App,
    ) -> Vec<PathBuf> {
        worktree_store
            .visible_worktrees(cx)
            .sorted_by(|left, right| {
                left.read(cx)
                    .is_single_file()
                    .cmp(&right.read(cx).is_single_file())
            })
            .filter_map(|worktree| {
                let worktree = worktree.read(cx);
                let path = worktree.abs_path();
                if worktree.is_single_file() {
                    Some(path.parent()?.to_path_buf())
                } else {
                    Some(path.to_path_buf())
                }
            })
            .collect()
    }
    pub fn default_path_list(&self, cx: &App) -> PathList {
        let worktree_roots =
            Self::default_visible_worktree_paths(&self.worktree_store.read(cx), cx);

        if worktree_roots.is_empty() {
            PathList::new(&[paths::home_dir().as_path()])
        } else {
            PathList::new(&worktree_roots)
        }
    }

    #[inline]
    pub fn worktree_for_root_name(&self, root_name: &str, cx: &App) -> Option<Entity<Worktree>> {
        self.visible_worktrees(cx)
            .find(|tree| tree.read(cx).root_name() == root_name)
    }
    pub fn worktree_root_names<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = &'a str> {
        self.visible_worktrees(cx)
            .map(|tree| tree.read(cx).root_name().as_unix_str())
    }

    #[inline]
    pub fn worktree_for_id(&self, id: WorktreeId, cx: &App) -> Option<Entity<Worktree>> {
        self.worktree_store.read(cx).worktree_for_id(id, cx)
    }
    pub fn worktree_for_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &App,
    ) -> Option<Entity<Worktree>> {
        self.worktree_store
            .read(cx)
            .worktree_for_entry(entry_id, cx)
    }

    #[inline]
    pub fn worktree_id_for_entry(&self, entry_id: ProjectEntryId, cx: &App) -> Option<WorktreeId> {
        self.worktree_for_entry(entry_id, cx)
            .map(|worktree| worktree.read(cx).id())
    }

    /// Checks if the entry is the root of a worktree.
    #[inline]
    pub fn entry_is_worktree_root(&self, entry_id: ProjectEntryId, cx: &App) -> bool {
        self.worktree_for_entry(entry_id, cx)
            .map(|worktree| {
                worktree
                    .read(cx)
                    .root_entry()
                    .is_some_and(|e| e.id == entry_id)
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn find_or_create_worktree(
        &mut self,
        abs_path: impl AsRef<Path>,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<(Entity<Worktree>, Arc<RelPath>)>> {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.find_or_create_worktree(abs_path, visible, cx)
        })
    }
    pub fn find_worktree(
        &self,
        abs_path: &Path,
        cx: &App,
    ) -> Option<(Entity<Worktree>, Arc<RelPath>)> {
        self.worktree_store.read(cx).find_worktree(abs_path, cx)
    }
    pub fn create_worktree(
        &mut self,
        abs_path: impl AsRef<Path>,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Worktree>>> {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.create_worktree(abs_path, visible, cx)
        })
    }

    /// Returns a task that resolves when the given worktree's `Entity` is
    /// fully dropped (all strong references released), not merely when
    /// `remove_worktree` is called. `remove_worktree` drops the store's
    /// reference and emits `WorktreeRemoved`, but other code may still
    /// hold a strong handle — the worktree isn't safe to delete from
    /// disk until every handle is gone.
    ///
    /// We use `observe_release` on the specific entity rather than
    /// listening for `WorktreeReleased` events because it's simpler at
    /// the call site (one awaitable task, no subscription / channel /
    /// ID filtering).
    pub fn wait_for_worktree_release(
        &mut self,
        worktree_id: WorktreeId,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let Some(worktree) = self.worktree_for_id(worktree_id, cx) else {
            return Task::ready(Ok(()));
        };

        let (released_tx, released_rx) = futures::channel::oneshot::channel();
        let released_tx = std::sync::Arc::new(Mutex::new(Some(released_tx)));
        let release_subscription =
            cx.observe_release(&worktree, move |_project, _released_worktree, _cx| {
                if let Some(released_tx) = released_tx.lock().take() {
                    let _ = released_tx.send(());
                }
            });

        cx.spawn(async move |_project, _cx| {
            let _release_subscription = release_subscription;
            released_rx
                .await
                .map_err(|_| anyhow!("worktree release observer dropped before release"))?;
            Ok(())
        })
    }
    pub fn remove_worktree(&mut self, id_to_remove: WorktreeId, cx: &mut Context<Self>) {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.remove_worktree(id_to_remove, cx);
        });
    }
    pub fn remove_worktree_for_main_worktree_path(
        &mut self,
        path: impl AsRef<Path>,
        cx: &mut Context<Self>,
    ) {
        let path = path.as_ref();
        self.worktree_store.update(cx, |worktree_store, cx| {
            if let Some(worktree) = worktree_store.worktree_for_main_worktree_path(path, cx) {
                worktree_store.remove_worktree(worktree.read(cx).id(), cx);
            }
        });
    }
    pub fn move_worktree(
        &mut self,
        source: WorktreeId,
        destination: WorktreeId,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.move_worktree(source, destination, cx)
        })
    }

    /// Attempts to convert the input path to a WSL path if this is a wsl remote project and the input path is a host windows path.
    fn add_worktree(&mut self, worktree: &Entity<Worktree>, cx: &mut Context<Self>) {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.add(worktree, cx);
        });
    }
    pub fn worktree_metadata_protos(&self, cx: &App) -> Vec<proto::WorktreeMetadata> {
        self.worktree_store.read(cx).worktree_metadata_protos(cx)
    }

    /// Iterator of all open buffers that have unsaved changes
    pub fn dirty_buffers<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = ProjectPath> + 'a {
        self.buffer_store.read(cx).buffers().filter_map(|buf| {
            let buf = buf.read(cx);
            if buf.is_dirty() {
                buf.project_path(cx)
            } else {
                None
            }
        })
    }
    pub fn worktree_paths(&self, cx: &App) -> WorktreePaths {
        self.worktree_store.read(cx).paths(cx)
    }
    pub fn path_style(&self, cx: &App) -> util::paths::PathStyle {
        self.worktree_store.read(cx).path_style()
    }
    pub fn contains_local_settings_file(
        &self,
        worktree_id: WorktreeId,
        rel_path: &RelPath,
        cx: &App,
    ) -> bool {
        self.worktree_for_id(worktree_id, cx)
            .map_or(false, |worktree| {
                worktree.read(cx).entry_for_path(rel_path).is_some()
            })
    }
    fn emit_group_key_changed_if_needed(&mut self, cx: &mut Context<Self>) {
        let new_worktree_paths = self.worktree_paths(cx);
        if new_worktree_paths != self.last_worktree_paths {
            let old_worktree_paths =
                std::mem::replace(&mut self.last_worktree_paths, new_worktree_paths);
            cx.emit(Event::WorktreePathsChanged { old_worktree_paths });
        }
    }

    #[inline]
    fn on_worktree_store_event(
        &mut self,
        _: Entity<WorktreeStore>,
        event: &WorktreeStoreEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorktreeStoreEvent::WorktreeAdded(worktree) => {
                self.on_worktree_added(worktree, cx);
                cx.emit(Event::WorktreeAdded(worktree.read(cx).id()));
                self.emit_group_key_changed_if_needed(cx);
            }
            WorktreeStoreEvent::WorktreeRemoved(_, id) => {
                cx.emit(Event::WorktreeRemoved(*id));
                self.emit_group_key_changed_if_needed(cx);
            }
            WorktreeStoreEvent::WorktreeReleased(_, id) => {
                self.on_worktree_released(*id, cx);
            }
            WorktreeStoreEvent::WorktreeOrderChanged => cx.emit(Event::WorktreeOrderChanged),
            WorktreeStoreEvent::WorktreeUpdateSent(_) => {}
            WorktreeStoreEvent::WorktreeUpdatedEntries(worktree_id, changes) => {
                self.client()
                    .telemetry()
                    .report_discovered_project_type_events(*worktree_id, changes);
                cx.emit(Event::WorktreeUpdatedEntries(*worktree_id, changes.clone()))
            }
            WorktreeStoreEvent::WorktreeDeletedEntry(worktree_id, id) => {
                cx.emit(Event::DeletedEntry(*worktree_id, *id))
            }
            // Listen to the GitStore instead.
            WorktreeStoreEvent::WorktreeUpdatedGitRepositories(_, _) => {}
            WorktreeStoreEvent::WorktreeUpdatedRootRepoCommonDir(worktree_id) => {
                cx.emit(Event::WorktreeUpdatedRootRepoCommonDir(*worktree_id));
                self.emit_group_key_changed_if_needed(cx);
            }
        }
    }
    fn on_worktree_added(&mut self, worktree: &Entity<Worktree>, _: &mut Context<Self>) {
        let mut remotely_created_models = self.remotely_created_models.lock();
        if remotely_created_models.retain_count > 0 {
            remotely_created_models.worktrees.push(worktree.clone())
        }
    }
    fn on_worktree_released(&mut self, id_to_remove: WorktreeId, cx: &mut Context<Self>) {
        if let Some(remote) = &self.remote_client {
            remote
                .read(cx)
                .proto_client()
                .send(proto::RemoveWorktree {
                    worktree_id: id_to_remove.to_proto(),
                })
                .log_err();
        }
    }
    fn set_worktrees_from_proto(
        &mut self,
        worktrees: Vec<proto::WorktreeMetadata>,
        cx: &mut Context<Project>,
    ) -> Result<()> {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.set_worktrees_from_proto(worktrees, self.replica_id(), cx)
        })
    }
    pub fn create_entry(
        &mut self,
        project_path: impl Into<ProjectPath>,
        is_directory: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<CreatedEntry>> {
        let project_path = project_path.into();
        let Some(worktree) = self.worktree_for_id(project_path.worktree_id, cx) else {
            return Task::ready(Err(anyhow!(format!(
                "No worktree for path {project_path:?}"
            ))));
        };
        worktree.update(cx, |worktree, cx| {
            worktree.create_entry(project_path.path, is_directory, None, cx)
        })
    }

    #[inline]
    pub fn copy_entry(
        &mut self,
        entry_id: ProjectEntryId,
        new_project_path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Entry>>> {
        self.worktree_store.update(cx, |worktree_store, cx| {
            worktree_store.copy_entry(entry_id, new_project_path, cx)
        })
    }

    /// Renames the project entry with given `entry_id`.
    ///
    /// `new_path` is a relative path to worktree root.
    /// If root entry is renamed then its new root name is used instead.
    pub fn rename_entry(
        &mut self,
        entry_id: ProjectEntryId,
        new_path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Task<Result<CreatedEntry>> {
        let worktree_store = self.worktree_store.clone();
        let Some((worktree, old_path, is_dir)) = worktree_store
            .read(cx)
            .worktree_and_entry_for_id(entry_id, cx)
            .map(|(worktree, entry)| (worktree, entry.path.clone(), entry.is_dir()))
        else {
            return Task::ready(Err(anyhow!(format!("No worktree for entry {entry_id:?}"))));
        };

        let worktree_id = worktree.read(cx).id();
        let is_root_entry = self.entry_is_worktree_root(entry_id, cx);

        let lsp_store = self.lsp_store().downgrade();
        cx.spawn(async move |project, cx| {
            let (old_abs_path, new_abs_path) = {
                let root_path = worktree.read_with(cx, |this, _| this.abs_path());
                let new_abs_path = if is_root_entry {
                    root_path
                        .parent()
                        .unwrap()
                        .join(new_path.path.as_std_path())
                } else {
                    root_path.join(&new_path.path.as_std_path())
                };
                (root_path.join(old_path.as_std_path()), new_abs_path)
            };
            let transaction = LspStore::will_rename_entry(
                lsp_store.clone(),
                worktree_id,
                &old_abs_path,
                &new_abs_path,
                is_dir,
                cx.clone(),
            )
            .await;

            let entry = worktree_store
                .update(cx, |worktree_store, cx| {
                    worktree_store.rename_entry(entry_id, new_path.clone(), cx)
                })
                .await?;

            project
                .update(cx, |_, cx| {
                    cx.emit(Event::EntryRenamed {
                        transaction,
                        new_project_path: new_path.clone(),
                        old_abs_path: old_abs_path.clone(),
                        new_abs_path: new_abs_path.clone(),
                    });
                })
                .ok();

            lsp_store
                .read_with(cx, |this, _| {
                    this.did_rename_entry(worktree_id, &old_abs_path, &new_abs_path, is_dir);
                })
                .ok();
            Ok(entry)
        })
    }

    #[inline]
    pub fn trash_file(
        &mut self,
        path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<TrashId>>> {
        let entry = self.entry_for_path(&path, cx)?;
        self.trash_entry(entry.id, cx)
    }

    #[inline]
    pub fn delete_file(
        &mut self,
        path: ProjectPath,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let entry = self.entry_for_path(&path, cx)?;
        self.delete_entry(entry.id, cx)
    }

    #[inline]
    pub fn trash_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<TrashId>>> {
        let worktree = self.worktree_for_entry(entry_id, cx)?;
        cx.emit(Event::DeletedEntry(worktree.read(cx).id(), entry_id));
        worktree.update(cx, |worktree, cx| worktree.trash_entry(entry_id, cx))
    }

    #[inline]
    pub fn delete_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let worktree = self.worktree_for_entry(entry_id, cx)?;
        cx.emit(Event::DeletedEntry(worktree.read(cx).id(), entry_id));
        worktree.update(cx, |worktree, cx| worktree.delete_entry(entry_id, cx))
    }

    #[inline]
    pub fn restore_entry(
        &self,
        worktree_id: WorktreeId,
        trash_id: TrashId,
        cx: &mut Context<'_, Self>,
    ) -> Task<Result<ProjectPath>> {
        let Some(worktree) = self.worktree_for_id(worktree_id, cx) else {
            return Task::ready(Err(anyhow!("No worktree for id {worktree_id:?}")));
        };

        cx.spawn(async move |_, cx| {
            let entry = worktree
                .update(cx, |worktree, cx| worktree.restore_entry(trash_id, cx))
                .await?;

            Ok(ProjectPath {
                worktree_id: worktree_id,
                path: entry.path,
            })
        })
    }

    #[inline]
    pub fn expand_entry(
        &mut self,
        worktree_id: WorktreeId,
        entry_id: ProjectEntryId,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let worktree = self.worktree_for_id(worktree_id, cx)?;
        worktree.update(cx, |worktree, cx| worktree.expand_entry(entry_id, cx))
    }
    pub fn expand_all_for_entry(
        &mut self,
        worktree_id: WorktreeId,
        entry_id: ProjectEntryId,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        let worktree = self.worktree_for_id(worktree_id, cx)?;
        let task = worktree.update(cx, |worktree, cx| {
            worktree.expand_all_for_entry(entry_id, cx)
        });
        Some(cx.spawn(async move |this, cx| {
            task.context("no task")?.await?;
            this.update(cx, |_, cx| {
                cx.emit(Event::ExpandedAllForEntry(worktree_id, entry_id));
            })?;
            Ok(())
        }))
    }
    pub fn entry_for_path<'a>(&'a self, path: &ProjectPath, cx: &'a App) -> Option<&'a Entry> {
        self.worktree_store.read(cx).entry_for_path(path, cx)
    }
    pub fn path_for_entry(&self, entry_id: ProjectEntryId, cx: &App) -> Option<ProjectPath> {
        let worktree = self.worktree_for_entry(entry_id, cx)?;
        let worktree = worktree.read(cx);
        let worktree_id = worktree.id();
        let path = worktree.entry_for_id(entry_id)?.path.clone();
        Some(ProjectPath { worktree_id, path })
    }
    pub fn absolute_path(&self, project_path: &ProjectPath, cx: &App) -> Option<PathBuf> {
        Some(
            self.worktree_for_id(project_path.worktree_id, cx)?
                .read(cx)
                .absolutize(&project_path.path),
        )
    }

    pub fn find_project_path(&self, path: impl AsRef<Path>, cx: &App) -> Option<ProjectPath> {
        let path_style = self.path_style(cx);
        let path = path.as_ref();
        let worktree_store = self.worktree_store.read(cx);

        if is_absolute(&path.to_string_lossy(), path_style) {
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
}
