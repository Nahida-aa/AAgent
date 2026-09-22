use super::*;
use postage::watch;
use rpc::proto;

pub struct LocalWorktree {
    pub(crate) snapshot: LocalSnapshot,
    pub(crate) scan_requests_tx: async_channel::Sender<ScanRequest>,
    pub(crate) path_prefixes_to_scan_tx: async_channel::Sender<PathPrefixScanRequest>,
    pub(crate) is_scanning: (watch::Sender<bool>, watch::Receiver<bool>),
    pub(crate) snapshot_subscriptions: VecDeque<(usize, oneshot::Sender<()>)>,
    pub(crate) _background_scanner_tasks: Vec<Task<()>>,
    pub(crate) update_observer: Option<UpdateObservationState>,
    pub(crate) fs: Arc<dyn Fs>,
    pub(crate) fs_case_sensitive: bool,
    pub(crate) visible: bool,
    pub(crate) next_entry_id: Arc<AtomicUsize>,
    pub(crate) settings: WorktreeSettings,
    pub(crate) share_private_files: bool,
    pub(crate) scanning_enabled: bool,
    pub(crate) force_defer_watch: bool,
}

pub struct PathPrefixScanRequest {
    path: Arc<RelPath>,
    done: SmallVec<[barrier::Sender; 1]>,
}

pub struct ScanRequest {
    relative_paths: Vec<Arc<RelPath>>,
    done: SmallVec<[barrier::Sender; 1]>,
}
impl LocalWorktree {
    pub fn fs(&self) -> &Arc<dyn Fs> { &self.fs }

    pub fn is_path_private(&self, path: &RelPath) -> bool {
        !self.share_private_files && self.settings.is_path_private(path)
    }

    pub fn fs_is_case_sensitive(&self) -> bool { self.fs_case_sensitive }

    pub(crate) fn restart_background_scanners(&mut self, cx: &Context<Worktree>) {
        let (scan_requests_tx, scan_requests_rx) = async_channel::unbounded();
        let (path_prefixes_to_scan_tx, path_prefixes_to_scan_rx) = async_channel::unbounded();
        self.scan_requests_tx = scan_requests_tx;
        self.path_prefixes_to_scan_tx = path_prefixes_to_scan_tx;

        self.start_background_scanner(scan_requests_rx, path_prefixes_to_scan_rx, cx);
        let always_included_entries = self
            .snapshot
            .entries(true, 0)
            .filter(|entry| entry.is_always_included)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        log::debug!(
            "refreshing entries for the following always included paths: {:?}",
            always_included_entries
        );

        // Cleans up old always included entries to ensure they get updated properly. Otherwise,
        // nested always included entries may not get updated and will result in out-of-date info.
        self.refresh_entries_for_paths(always_included_entries);
    }

    pub(crate) fn start_background_scanner(
        &mut self,
        scan_requests_rx: async_channel::Receiver<ScanRequest>,
        path_prefixes_to_scan_rx: async_channel::Receiver<PathPrefixScanRequest>,
        cx: &Context<Worktree>,
    ) {
        let snapshot = self.snapshot();
        let share_private_files = self.share_private_files;
        let next_entry_id = self.next_entry_id.clone();
        let fs = self.fs.clone();
        let scanning_enabled = self.scanning_enabled;
        let force_defer_watch = self.force_defer_watch;
        let track_git_repositories = self.visible;
        let settings = self.settings.clone();
        let (scan_states_tx, mut scan_states_rx) = mpsc::unbounded();
        let background_scanner = cx.background_spawn({
            let abs_path = snapshot.abs_path.as_path().to_path_buf();
            let background = cx.background_executor().clone();
            async move {
                let defer_watch =
                    force_defer_watch || (scanning_enabled && fs.requires_poll_watcher(&abs_path));

                let (events, watcher) = if scanning_enabled && !defer_watch {
                    fs.watch(&abs_path, FS_WATCH_LATENCY).await
                } else {
                    (Box::pin(stream::pending()) as _, Arc::new(NullWatcher) as _)
                };
                let fs_case_sensitive = fs.is_case_sensitive().await;

                let is_single_file = snapshot.snapshot.root_dir().is_none();
                let mut scanner = BackgroundScanner {
                    fs,
                    fs_case_sensitive,
                    status_updates_tx: scan_states_tx,
                    executor: background,
                    scan_requests_rx,
                    path_prefixes_to_scan_rx,
                    next_entry_id,
                    state: async_lock::Mutex::new(BackgroundScannerState {
                        prev_snapshot: snapshot.snapshot.clone(),
                        snapshot,
                        symlink_paths_by_target: Default::default(),
                        scanned_dirs: Default::default(),
                        watched_dir_abs_paths_by_entry_id: Default::default(),
                        scanning_enabled,
                        path_prefixes_to_scan: Default::default(),
                        paths_to_scan: Default::default(),
                        removed_entries: RemovedEntries::default(),
                        changed_paths: Default::default(),
                    }),
                    phase: BackgroundScannerPhase::InitialScan,
                    share_private_files,
                    settings,
                    watcher,
                    track_git_repositories,
                    is_single_file,
                    defer_watch,
                };

                scanner.run(events).await;
            }
        });
        let scan_state_updater = cx.spawn(async move |this, cx| {
            while let Some((state, this)) = scan_states_rx.next().await.zip(this.upgrade()) {
                this.update(cx, |this, cx| {
                    let this = this.as_local_mut().unwrap();
                    match state {
                        ScanState::Started => {
                            *this.is_scanning.0.borrow_mut() = true;
                        }
                        ScanState::Updated {
                            snapshot,
                            changes,
                            barrier,
                            scanning,
                        } => {
                            *this.is_scanning.0.borrow_mut() = scanning;
                            this.set_snapshot(snapshot, changes, cx);
                            drop(barrier);
                        }
                        ScanState::RootUpdated { new_path } => {
                            this.update_abs_path_and_refresh(new_path, cx);
                        }
                        ScanState::RootDeleted => {
                            log::info!(
                                "worktree root {} no longer exists, closing worktree",
                                this.abs_path().display()
                            );
                            cx.emit(Event::Deleted);
                        }
                    }
                });
            }
        });
        self._background_scanner_tasks = vec![background_scanner, scan_state_updater];
        *self.is_scanning.0.borrow_mut() = true;
    }

    fn set_snapshot(
        &mut self,
        mut new_snapshot: LocalSnapshot,
        entry_changes: UpdatedEntriesSet,
        cx: &mut Context<Worktree>,
    ) {
        let repo_changes = self.changed_repos(&self.snapshot, &mut new_snapshot);

        if let Some((common_dir, is_linked_worktree)) = new_snapshot
            .local_repo_for_work_directory_path(RelPath::empty())
            .map(|repo| {
                (
                    SanitizedPath::from_arc(repo.common_dir_abs_path.clone()),
                    repo.repository_dir_abs_path != repo.common_dir_abs_path,
                )
            })
        {
            new_snapshot.root_repo_common_dir = Some(common_dir);
            new_snapshot.root_repo_is_linked_worktree = is_linked_worktree;
        } else {
            new_snapshot.root_repo_common_dir = None;
            new_snapshot.root_repo_is_linked_worktree = false;
        }

        let root_repo_metadata_changed = self.snapshot.root_repo_common_dir
            != new_snapshot.root_repo_common_dir
            || self.snapshot.root_repo_is_linked_worktree
                != new_snapshot.root_repo_is_linked_worktree;
        let old_root_repo_common_dir =
            root_repo_metadata_changed.then(|| self.snapshot.root_repo_common_dir.clone());
        self.snapshot = new_snapshot;

        if let Some(share) = self.update_observer.as_mut() {
            share
                .snapshots_tx
                .unbounded_send((self.snapshot.clone(), entry_changes.clone()))
                .ok();
        }

        if !entry_changes.is_empty() {
            cx.emit(Event::UpdatedEntries(entry_changes));
        }
        if !repo_changes.is_empty() {
            cx.emit(Event::UpdatedGitRepositories(repo_changes));
        }
        if let Some(old) = old_root_repo_common_dir {
            cx.emit(Event::UpdatedRootRepoCommonDir { old });
        }

        while let Some((scan_id, _)) = self.snapshot_subscriptions.front() {
            if self.snapshot.completed_scan_id >= *scan_id {
                let (_, tx) = self.snapshot_subscriptions.pop_front().unwrap();
                tx.send(()).ok();
            } else {
                break;
            }
        }
    }

    fn changed_repos(
        &self,
        old_snapshot: &LocalSnapshot,
        new_snapshot: &mut LocalSnapshot,
    ) -> UpdatedGitRepositoriesSet {
        let mut changes = Vec::new();
        let mut old_repos = old_snapshot.git_repositories.iter().peekable();
        let new_repos = new_snapshot.git_repositories.clone();
        let mut new_repos = new_repos.iter().peekable();

        loop {
            match (new_repos.peek().map(clone), old_repos.peek().map(clone)) {
                (Some((new_entry_id, new_repo)), Some((old_entry_id, old_repo))) => {
                    match Ord::cmp(&new_entry_id, &old_entry_id) {
                        Ordering::Less => {
                            changes.push(UpdatedGitRepository {
                                work_directory_id: new_entry_id,
                                old_work_directory_abs_path: None,
                                new_work_directory_abs_path: Some(
                                    new_repo.work_directory_abs_path.clone(),
                                ),
                                dot_git_abs_path: Some(new_repo.dot_git_abs_path.clone()),
                                repository_dir_abs_path: Some(
                                    new_repo.repository_dir_abs_path.clone(),
                                ),
                                common_dir_abs_path: Some(new_repo.common_dir_abs_path.clone()),
                            });
                            new_repos.next();
                        }
                        Ordering::Equal => {
                            // A change to a repository's git state is signaled by bumping
                            // `git_dir_scan_id`, and the diff below detects it via `!=`. If the
                            // value ever regresses (e.g. a rescan re-inserting the repository
                            // with a fresh scan id of 0), a bump from the same cycle is wiped
                            // out and the corresponding git update is silently lost.
                            debug_assert!(
                                new_repo.git_dir_scan_id >= old_repo.git_dir_scan_id,
                                "git_dir_scan_id for repository at {:?} regressed from {} to {}",
                                new_repo.work_directory_abs_path,
                                old_repo.git_dir_scan_id,
                                new_repo.git_dir_scan_id,
                            );
                            if new_repo.git_dir_scan_id != old_repo.git_dir_scan_id
                                || new_repo.work_directory_abs_path
                                    != old_repo.work_directory_abs_path
                            {
                                changes.push(UpdatedGitRepository {
                                    work_directory_id: new_entry_id,
                                    old_work_directory_abs_path: Some(
                                        old_repo.work_directory_abs_path.clone(),
                                    ),
                                    new_work_directory_abs_path: Some(
                                        new_repo.work_directory_abs_path.clone(),
                                    ),
                                    dot_git_abs_path: Some(new_repo.dot_git_abs_path.clone()),
                                    repository_dir_abs_path: Some(
                                        new_repo.repository_dir_abs_path.clone(),
                                    ),
                                    common_dir_abs_path: Some(new_repo.common_dir_abs_path.clone()),
                                });
                            }
                            new_repos.next();
                            old_repos.next();
                        }
                        Ordering::Greater => {
                            changes.push(UpdatedGitRepository {
                                work_directory_id: old_entry_id,
                                old_work_directory_abs_path: Some(
                                    old_repo.work_directory_abs_path.clone(),
                                ),
                                new_work_directory_abs_path: None,
                                dot_git_abs_path: None,
                                repository_dir_abs_path: None,
                                common_dir_abs_path: None,
                            });
                            old_repos.next();
                        }
                    }
                }
                (Some((entry_id, repo)), None) => {
                    changes.push(UpdatedGitRepository {
                        work_directory_id: entry_id,
                        old_work_directory_abs_path: None,
                        new_work_directory_abs_path: Some(repo.work_directory_abs_path.clone()),
                        dot_git_abs_path: Some(repo.dot_git_abs_path.clone()),
                        repository_dir_abs_path: Some(repo.repository_dir_abs_path.clone()),
                        common_dir_abs_path: Some(repo.common_dir_abs_path.clone()),
                    });
                    new_repos.next();
                }
                (None, Some((entry_id, repo))) => {
                    changes.push(UpdatedGitRepository {
                        work_directory_id: entry_id,
                        old_work_directory_abs_path: Some(repo.work_directory_abs_path.clone()),
                        new_work_directory_abs_path: None,
                        dot_git_abs_path: Some(repo.dot_git_abs_path.clone()),
                        repository_dir_abs_path: Some(repo.repository_dir_abs_path.clone()),
                        common_dir_abs_path: Some(repo.common_dir_abs_path.clone()),
                    });
                    old_repos.next();
                }
                (None, None) => break,
            }
        }

        fn clone<T: Clone, U: Clone>(value: &(&T, &U)) -> (T, U) {
            (value.0.clone(), value.1.clone())
        }

        changes.into()
    }

    pub fn scan_complete(&self) -> impl Future<Output = ()> + use<> {
        let mut is_scanning_rx = self.is_scanning.1.clone();
        async move {
            let mut is_scanning = *is_scanning_rx.borrow();
            while is_scanning {
                if let Some(value) = is_scanning_rx.recv().await {
                    is_scanning = value;
                } else {
                    break;
                }
            }
        }
    }

    pub fn wait_for_snapshot(
        &mut self,
        scan_id: usize,
    ) -> impl Future<Output = Result<()>> + use<> {
        let (tx, rx) = oneshot::channel();
        if self.snapshot.completed_scan_id >= scan_id {
            tx.send(()).ok();
        } else {
            match self
                .snapshot_subscriptions
                .binary_search_by_key(&scan_id, |probe| probe.0)
            {
                Ok(ix) | Err(ix) => self.snapshot_subscriptions.insert(ix, (scan_id, tx)),
            }
        }

        async move {
            rx.await?;
            Ok(())
        }
    }

    pub fn snapshot(&self) -> LocalSnapshot { self.snapshot.clone() }

    pub fn settings(&self) -> WorktreeSettings { self.settings.clone() }

    pub(crate) fn load_binary_file(
        &self,
        path: &RelPath,
        cx: &Context<Worktree>,
    ) -> Task<Result<LoadedBinaryFile>> {
        let path = Arc::from(path);
        let abs_path = self.absolutize(&path);
        let fs = self.fs.clone();
        let entry = self.refresh_entry(path.clone(), None, cx);
        let is_private = self.is_path_private(&path);

        let worktree = cx.weak_entity();
        cx.background_spawn(async move {
            let content = fs.load_bytes(&abs_path).await?;

            let worktree = worktree.upgrade().context("worktree was dropped")?;
            let file = match entry.await? {
                Some(entry) => File::for_entry(entry, worktree),
                None => {
                    let metadata = fs
                        .metadata(&abs_path)
                        .await
                        .with_context(|| {
                            format!("Loading metadata for excluded file {abs_path:?}")
                        })?
                        .with_context(|| {
                            format!("Excluded file {abs_path:?} got removed during loading")
                        })?;
                    Arc::new(File {
                        entry_id: None,
                        worktree,
                        path,
                        disk_state: DiskState::Present {
                            mtime: metadata.mtime,
                            size: metadata.len,
                        },
                        is_local: true,
                        is_private,
                    })
                }
            };

            Ok(LoadedBinaryFile { file, content })
        })
    }

    #[a_tracing::instrument(skip_all)]
    pub(crate) fn load_file(
        &self,
        path: &RelPath,
        cx: &Context<Worktree>,
    ) -> Task<Result<LoadedFile>> {
        let path = Arc::from(path);
        let abs_path = self.absolutize(&path);
        let fs = self.fs.clone();
        let entry = self.refresh_entry(path.clone(), None, cx);
        let is_private = self.is_path_private(path.as_ref());

        let this = cx.weak_entity();
        cx.background_spawn(async move {
            // WARN: Temporary workaround for #27283.
            //       We are not efficient with our memory usage per file, and use in excess of 64GB for a 10GB file
            //       Therefore, as a temporary workaround to prevent system freezes, we just bail before opening a file
            //       if it is too large
            //       5GB seems to be more reasonable, peaking at ~16GB, while 6GB jumps up to >24GB which seems like a
            //       reasonable limit
            const FILE_SIZE_MAX: u64 = 6 * 1024 * 1024 * 1024; // 6GB
            let metadata = fs.metadata(&abs_path).await?;
            if let Some(metadata) = metadata.as_ref()
                && metadata.len >= FILE_SIZE_MAX
            {
                anyhow::bail!("File is too large to load");
            }
            let (text, line_ending, encoding, has_bom) =
                decode_file_text_to_rope(fs.as_ref(), &abs_path).await?;
            let is_writable = metadata.is_some_and(|metadata| metadata.is_writable);

            let worktree = this.upgrade().context("worktree was dropped")?;
            let file = match entry.await? {
                Some(entry) => File::for_entry(entry, worktree),
                None => {
                    let metadata = fs
                        .metadata(&abs_path)
                        .await
                        .with_context(|| {
                            format!("Loading metadata for excluded file {abs_path:?}")
                        })?
                        .with_context(|| {
                            format!("Excluded file {abs_path:?} got removed during loading")
                        })?;
                    Arc::new(File {
                        entry_id: None,
                        worktree,
                        path,
                        disk_state: DiskState::Present {
                            mtime: metadata.mtime,
                            size: metadata.len,
                        },
                        is_local: true,
                        is_private,
                    })
                }
            };

            Ok(LoadedFile {
                file,
                text,
                line_ending,
                encoding,
                has_bom,
                is_writable,
            })
        })
    }

    /// Find the lowest path in the worktree's datastructures that is an ancestor
    fn lowest_ancestor(&self, path: &RelPath) -> Arc<RelPath> {
        let mut lowest_ancestor = None;
        for path in path.ancestors() {
            if self.entry_for_path(path).is_some() {
                lowest_ancestor = Some(path.into());
                break;
            }
        }

        lowest_ancestor.unwrap_or_else(|| RelPath::empty_arc())
    }

    pub fn create_entry(
        &self,
        path: Arc<RelPath>,
        is_dir: bool,
        content: Option<Vec<u8>>,
        cx: &Context<Worktree>,
    ) -> Task<Result<CreatedEntry>> {
        let abs_path = self.absolutize(&path);
        let path_excluded = self.settings.is_path_excluded(&path);
        let fs = self.fs.clone();
        let task_abs_path = abs_path.clone();
        let write = cx.background_spawn(async move {
            if is_dir {
                fs.create_dir(&task_abs_path)
                    .await
                    .with_context(|| format!("creating directory {task_abs_path:?}"))
            } else {
                fs.write(&task_abs_path, content.as_deref().unwrap_or(&[]))
                    .await
                    .with_context(|| format!("creating file {task_abs_path:?}"))
            }
        });

        let lowest_ancestor = self.lowest_ancestor(&path);
        cx.spawn(async move |this, cx| {
            write.await?;
            if path_excluded {
                return Ok(CreatedEntry::Excluded { abs_path });
            }

            let (result, refreshes) = this.update(cx, |this, cx| {
                let mut refreshes = Vec::new();
                let refresh_paths = path.strip_prefix(&lowest_ancestor).unwrap();
                for refresh_path in refresh_paths.ancestors() {
                    if refresh_path == RelPath::empty() {
                        continue;
                    }
                    let refresh_full_path = lowest_ancestor.join(refresh_path);

                    refreshes.push(this.as_local_mut().unwrap().refresh_entry(
                        refresh_full_path.into(),
                        None,
                        cx,
                    ));
                }
                (
                    this.as_local_mut().unwrap().refresh_entry(path, None, cx),
                    refreshes,
                )
            })?;
            for refresh in refreshes {
                refresh.await.log_err();
            }

            Ok(result
                .await?
                .map(CreatedEntry::Included)
                .unwrap_or_else(|| CreatedEntry::Excluded { abs_path }))
        })
    }

    pub fn write_file(
        &self,
        path: Arc<RelPath>,
        text: Rope,
        line_ending: LineEnding,
        encoding: &'static Encoding,
        has_bom: bool,
        cx: &Context<Worktree>,
    ) -> Task<Result<Arc<File>>> {
        let fs = self.fs.clone();
        let is_private = self.is_path_private(&path);
        let abs_path = self.absolutize(&path);

        let write = cx.background_spawn({
            let fs = fs.clone();
            let abs_path = abs_path.clone();
            async move {
                // For UTF-8, use the optimized `fs.save` which writes Rope chunks directly to disk
                // without allocating a contiguous string.
                if encoding == encoding_rs::UTF_8 && !has_bom {
                    return fs.save(&abs_path, &text, line_ending).await;
                }

                // For legacy encodings (e.g. Shift-JIS), we fall back to converting the entire Rope
                // to a String/Bytes in memory before writing.
                //
                // Note: This is inefficient for very large files compared to the streaming approach above,
                // but supporting streaming writes for arbitrary encodings would require a significant
                // refactor of the `fs` crate to expose a Writer interface.
                let text_string = text.to_string();
                let normalized_text = match line_ending {
                    LineEnding::Unix => text_string,
                    LineEnding::Windows => text_string.replace('\n', "\r\n"),
                };

                let bytes = encode_text(normalized_text, encoding, has_bom);
                fs.write(&abs_path, &bytes).await
            }
        });

        cx.spawn(async move |this, cx| {
            write.await?;
            let entry = this
                .update(cx, |this, cx| {
                    this.as_local_mut()
                        .unwrap()
                        .refresh_entry(path.clone(), None, cx)
                })?
                .await?;
            let worktree = this.upgrade().context("worktree dropped")?;
            if let Some(entry) = entry {
                Ok(File::for_entry(entry, worktree))
            } else {
                let metadata = fs
                    .metadata(&abs_path)
                    .await
                    .with_context(|| {
                        format!("Fetching metadata after saving the excluded buffer {abs_path:?}")
                    })?
                    .with_context(|| {
                        format!("Excluded buffer {path:?} got removed during saving")
                    })?;
                Ok(Arc::new(File {
                    worktree,
                    path,
                    disk_state: DiskState::Present {
                        mtime: metadata.mtime,
                        size: metadata.len,
                    },
                    entry_id: None,
                    is_local: true,
                    is_private,
                }))
            }
        })
    }

    pub fn trash_entry(&self, entry: Entry, cx: &Context<Worktree>) -> Task<Result<TrashId>> {
        let abs_path = self.absolutize(&entry.path);
        let fs = self.fs.clone();

        cx.spawn(async move |this, cx| {
            let trash_id = if entry.is_file() {
                fs.trash(&abs_path, Default::default()).await?
            } else {
                fs.trash(
                    &abs_path,
                    RemoveOptions {
                        recursive: true,
                        ignore_if_not_exists: false,
                    },
                )
                .await?
            };

            this.update(cx, |this, _| {
                this.as_local_mut()
                    .unwrap()
                    .refresh_entries_for_paths(vec![entry.path])
            })?
            .recv()
            .await;

            Ok(trash_id)
        })
    }

    pub fn delete_entry(&self, entry: Entry, cx: &Context<Worktree>) -> Task<Result<()>> {
        let abs_path = self.absolutize(&entry.path);
        let fs = self.fs.clone();

        cx.spawn(async move |this, cx| {
            if entry.is_file() {
                fs.remove_file(&abs_path, Default::default()).await?
            } else {
                fs.remove_dir(
                    &abs_path,
                    RemoveOptions {
                        recursive: true,
                        ignore_if_not_exists: false,
                    },
                )
                .await?
            };

            this.update(cx, |this, _| {
                this.as_local_mut()
                    .unwrap()
                    .refresh_entries_for_paths(vec![entry.path])
            })?
            .recv()
            .await;

            Ok(())
        })
    }

    pub fn restore_entry(
        &mut self,
        trash_id: TrashId,
        cx: &mut Context<'_, Worktree>,
    ) -> Task<Result<Entry>> {
        let fs = self.fs.clone();
        let worktree_abs_path = self.abs_path().clone();
        let path_style = self.path_style();

        cx.spawn(async move |this, cx| {
            let path_buf = fs.restore(trash_id).await?;
            let path = path_buf
                .strip_prefix(worktree_abs_path)
                .context("Could not strip prefix")?;
            let path = Arc::from(RelPath::new(&path, path_style)?.as_ref());

            let entry = this
                .update(cx, |this, cx| {
                    this.as_local_mut().unwrap().refresh_entry(path, None, cx)
                })?
                .await?
                .context("Entry not found after restore")?;

            Ok(entry)
        })
    }

    pub fn copy_external_entries(
        &self,
        target_directory: Arc<RelPath>,
        paths: Vec<Arc<Path>>,
        cx: &Context<Worktree>,
    ) -> Task<Result<Vec<ProjectEntryId>>> {
        let target_directory = self.absolutize(&target_directory);
        let worktree_path = self.abs_path().clone();
        let fs = self.fs.clone();
        let paths = paths
            .into_iter()
            .filter_map(|source| {
                let file_name = source.file_name()?;
                let mut target = target_directory.clone();
                target.push(file_name);

                // Do not allow copying the same file to itself.
                if source.as_ref() != target.as_path() {
                    Some((source, target))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let paths_to_refresh = paths
            .iter()
            .filter_map(|(_, target)| {
                RelPath::new(
                    target.strip_prefix(&worktree_path).ok()?,
                    PathStyle::local(),
                )
                .ok()
                .map(|path| path.into_arc())
            })
            .collect::<Vec<_>>();

        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                for (source, target) in paths {
                    copy_recursive(
                        fs.as_ref(),
                        &source,
                        &target,
                        fs::CopyOptions {
                            overwrite: true,
                            ..Default::default()
                        },
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to copy file from {source:?} to {target:?}")
                    })?;
                }
                anyhow::Ok(())
            })
            .await
            .log_err();
            let mut refresh = cx.read_entity(
                &this.upgrade().with_context(|| "Dropped worktree")?,
                |this, _| {
                    anyhow::Ok::<postage::barrier::Receiver>(
                        this.as_local()
                            .with_context(|| "Worktree is not local")?
                            .refresh_entries_for_paths(paths_to_refresh.clone()),
                    )
                },
            )?;

            cx.background_spawn(async move {
                refresh.next().await;
                anyhow::Ok(())
            })
            .await
            .log_err();

            let this = this.upgrade().with_context(|| "Dropped worktree")?;
            Ok(cx.read_entity(&this, |this, _| {
                paths_to_refresh
                    .iter()
                    .filter_map(|path| Some(this.entry_for_path(path)?.id))
                    .collect()
            }))
        })
    }

    pub(crate) fn expand_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Option<Task<Result<()>>> {
        let path = self.entry_for_id(entry_id)?.path.clone();
        let mut refresh = self.refresh_entries_for_paths(vec![path]);
        Some(cx.background_spawn(async move {
            refresh.next().await;
            Ok(())
        }))
    }

    pub(crate) fn expand_all_for_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Option<Task<Result<()>>> {
        let path = self.entry_for_id(entry_id).unwrap().path.clone();
        let mut rx = self.add_path_prefix_to_scan(path);
        Some(cx.background_spawn(async move {
            rx.next().await;
            Ok(())
        }))
    }

    pub fn refresh_entries_for_paths(&self, paths: Vec<Arc<RelPath>>) -> barrier::Receiver {
        let (tx, rx) = barrier::channel();
        self.scan_requests_tx
            .try_send(ScanRequest {
                relative_paths: paths,
                done: smallvec![tx],
            })
            .ok();
        rx
    }

    #[cfg(feature = "test-support")]
    pub fn manually_refresh_entries_for_paths(
        &self,
        paths: Vec<Arc<RelPath>>,
    ) -> barrier::Receiver {
        self.refresh_entries_for_paths(paths)
    }

    pub fn add_path_prefix_to_scan(&self, path_prefix: Arc<RelPath>) -> barrier::Receiver {
        let (tx, rx) = barrier::channel();
        self.path_prefixes_to_scan_tx
            .try_send(PathPrefixScanRequest {
                path: path_prefix,
                done: smallvec![tx],
            })
            .ok();
        rx
    }

    pub fn refresh_entry(
        &self,
        path: Arc<RelPath>,
        old_path: Option<Arc<RelPath>>,
        cx: &Context<Worktree>,
    ) -> Task<Result<Option<Entry>>> {
        if self.settings.is_path_excluded(&path) {
            return Task::ready(Ok(None));
        }
        let paths = if let Some(old_path) = old_path.as_ref() {
            vec![old_path.clone(), path.clone()]
        } else {
            vec![path.clone()]
        };
        let t0 = Instant::now();
        let mut refresh = self.refresh_entries_for_paths(paths);
        // todo(lw): Hot foreground spawn
        cx.spawn(async move |this, cx| {
            refresh.recv().await;
            log::trace!("refreshed entry {path:?} in {:?}", t0.elapsed());
            let new_entry = this.read_with(cx, |this, _| {
                this.entry_for_path(&path).cloned().with_context(|| {
                    format!("Could not find entry in worktree for {path:?} after refresh")
                })
            })??;
            Ok(Some(new_entry))
        })
    }

    pub fn observe_updates<F, Fut>(&mut self, project_id: u64, cx: &Context<Worktree>, callback: F)
    where
        F: 'static + Send + Fn(proto::UpdateWorktree) -> Fut,
        Fut: 'static + Send + Future<Output = bool>,
    {
        if let Some(observer) = self.update_observer.as_mut() {
            *observer.resume_updates.borrow_mut() = ();
            return;
        }

        let (resume_updates_tx, mut resume_updates_rx) = watch::channel::<()>();
        let (snapshots_tx, mut snapshots_rx) =
            mpsc::unbounded::<(LocalSnapshot, UpdatedEntriesSet)>();
        snapshots_tx
            .unbounded_send((self.snapshot(), Arc::default()))
            .ok();

        let worktree_id = self.id.to_proto();
        let _maintain_remote_snapshot = cx.background_spawn(async move {
            let mut is_first = true;
            while let Some((snapshot, entry_changes)) = snapshots_rx.next().await {
                let update = if is_first {
                    is_first = false;
                    snapshot.build_initial_update(project_id, worktree_id)
                } else {
                    snapshot.build_update(project_id, worktree_id, entry_changes)
                };

                for update in proto::split_worktree_update(update) {
                    let _ = resume_updates_rx.try_recv();
                    loop {
                        let result = callback(update.clone());
                        if result.await {
                            break;
                        } else {
                            log::info!("waiting to resume updates");
                            if resume_updates_rx.next().await.is_none() {
                                return Some(());
                            }
                        }
                    }
                }
            }
            Some(())
        });

        self.update_observer = Some(UpdateObservationState {
            snapshots_tx,
            resume_updates: resume_updates_tx,
            _maintain_remote_snapshot,
        });
    }

    pub fn share_private_files(&mut self, cx: &Context<Worktree>) {
        self.share_private_files = true;
        self.restart_background_scanners(cx);
    }

    pub fn update_abs_path_and_refresh(
        &mut self,
        new_path: Arc<SanitizedPath>,
        cx: &Context<Worktree>,
    ) {
        self.snapshot.git_repositories = Default::default();
        self.snapshot.ignores_by_parent_abs_path = Default::default();
        let root_name = new_path
            .as_path()
            .file_name()
            .and_then(|f| f.to_str())
            .map_or(RelPath::empty_arc(), |f| {
                RelPath::from_unix_str(f).unwrap().into()
            });
        self.snapshot.update_abs_path(new_path, root_name);
        self.restart_background_scanners(cx);
    }
    #[cfg(feature = "test-support")]
    pub fn set_defer_watch(&mut self, defer: bool, cx: &mut Context<Worktree>) {
        self.force_defer_watch = defer;
        self.restart_background_scanners(cx);
    }

    #[cfg(feature = "test-support")]
    pub fn repositories(&self) -> Vec<Arc<Path>> {
        self.git_repositories
            .values()
            .map(|entry| entry.work_directory_abs_path.clone())
            .collect::<Vec<_>>()
    }
}

impl Deref for LocalWorktree {
    type Target = LocalSnapshot;
    fn deref(&self) -> &Self::Target { &self.snapshot }
}

impl fmt::Debug for LocalWorktree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.snapshot.fmt(f) }
}
