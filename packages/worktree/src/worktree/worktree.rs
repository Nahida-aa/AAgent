use super::*;

pub enum Worktree {
    Local(LocalWorktree),
    Remote(RemoteWorktree),
}

impl EventEmitter<Event> for Worktree {}

impl Worktree {
    pub async fn local(
        path: impl Into<Arc<Path>>,
        visible: bool,
        fs: Arc<dyn Fs>,
        next_entry_id: Arc<AtomicUsize>,
        scanning_enabled: bool,
        worktree_id: WorktreeId,
        cx: &mut AsyncApp,
    ) -> Result<Entity<Self>> {
        let abs_path = path.into();
        let metadata = fs
            .metadata(&abs_path)
            .await
            .context("failed to stat worktree path")?;

        let fs_case_sensitive = fs.is_case_sensitive().await;

        let root_file_handle = if metadata.as_ref().is_some() {
            fs.open_handle(&abs_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to open local worktree root at {}",
                        abs_path.display()
                    )
                })
                .log_err()
        } else {
            None
        };

        let (root_repo_common_dir, root_repo_is_linked_worktree) = if visible {
            discover_root_repo_metadata(&abs_path, fs.as_ref())
                .await
                .map(|(common_dir, is_linked_worktree)| {
                    (
                        Some(SanitizedPath::from_arc(common_dir)),
                        is_linked_worktree,
                    )
                })
                .unwrap_or((None, false))
        } else {
            (None, false)
        };
        Ok(cx.new(move |cx: &mut Context<Worktree>| {
            let mut snapshot = LocalSnapshot {
                ignores_by_parent_abs_path: Default::default(),
                global_gitignore: Default::default(),
                repo_exclude_by_work_dir_abs_path: Default::default(),
                git_repositories: Default::default(),
                external_canonical_to_relative: Default::default(),
                snapshot: Snapshot::new(
                    worktree_id,
                    abs_path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .map_or(RelPath::empty_arc(), |f| {
                            RelPath::from_unix_str(f).unwrap().into()
                        }),
                    abs_path.clone(),
                    PathStyle::local(),
                ),
                root_file_handle,
            };
            snapshot.root_repo_common_dir = root_repo_common_dir;
            snapshot.root_repo_is_linked_worktree = root_repo_is_linked_worktree;

            let worktree_id = snapshot.id();
            let settings_location = Some(SettingsLocation {
                worktree_id,
                path: RelPath::empty(),
            });

            let settings = WorktreeSettings::get(settings_location, cx).clone();
            cx.observe_global::<SettingsStore>(move |this, cx| {
                if let Self::Local(this) = this {
                    let settings = WorktreeSettings::get(settings_location, cx).clone();
                    if this.settings != settings {
                        this.settings = settings;
                        this.restart_background_scanners(cx);
                    }
                }
            })
            .detach();

            let share_private_files = false;
            if let Some(metadata) = metadata {
                let mut entry = Entry::new(
                    RelPath::empty_arc(),
                    &metadata,
                    ProjectEntryId::new(&next_entry_id),
                    snapshot.root_char_bag,
                    None,
                );
                if metadata.is_dir {
                    if !scanning_enabled {
                        entry.kind = EntryKind::UnloadedDir;
                    }
                } else {
                    if let Some(file_name) = abs_path.file_name()
                        && let Some(file_name) = file_name.to_str()
                        && let Ok(path) = RelPath::from_unix_str(file_name)
                    {
                        entry.is_private = !share_private_files && settings.is_path_private(path);
                        entry.is_hidden = settings.is_path_hidden(path);
                    }
                }
                cx.foreground_executor()
                    .block_on(snapshot.insert_entry(entry, fs.as_ref()));
            }

            let (scan_requests_tx, scan_requests_rx) = async_channel::unbounded();
            let (path_prefixes_to_scan_tx, path_prefixes_to_scan_rx) = async_channel::unbounded();
            let mut worktree = LocalWorktree {
                share_private_files,
                next_entry_id,
                snapshot,
                is_scanning: watch::channel_with(true),
                snapshot_subscriptions: Default::default(),
                update_observer: None,
                scan_requests_tx,
                path_prefixes_to_scan_tx,
                _background_scanner_tasks: Vec::new(),
                fs,
                fs_case_sensitive,
                visible,
                settings,
                scanning_enabled,
                force_defer_watch: false,
            };
            worktree.start_background_scanner(scan_requests_rx, path_prefixes_to_scan_rx, cx);
            Worktree::Local(worktree)
        }))
    }

    pub fn remote(
        project_id: u64,
        replica_id: ReplicaId,
        worktree: proto::WorktreeMetadata,
        client: AnyProtoClient,
        path_style: PathStyle,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let mut snapshot = Snapshot::new(
                WorktreeId::from_proto(worktree.id),
                RelPath::from_unix_str(&worktree.root_name)
                    .map_or_else(|_| RelPath::empty_arc(), Into::into),
                Path::new(&worktree.abs_path).into(),
                path_style,
            );

            snapshot.root_repo_common_dir = worktree
                .root_repo_common_dir
                .map(|p| SanitizedPath::new_arc(Path::new(&p)));
            snapshot.root_repo_is_linked_worktree = worktree.root_repo_is_linked_worktree;

            let background_snapshot = Arc::new(Mutex::new((
                snapshot.clone(),
                Vec::<proto::UpdateWorktree>::new(),
            )));
            let (background_updates_tx, mut background_updates_rx) =
                mpsc::unbounded::<proto::UpdateWorktree>();
            let (mut snapshot_updated_tx, mut snapshot_updated_rx) = watch::channel();

            let worktree_id = snapshot.id();
            let settings_location = Some(SettingsLocation {
                worktree_id,
                path: RelPath::empty(),
            });

            let settings = WorktreeSettings::get(settings_location, cx).clone();
            let worktree = RemoteWorktree {
                client,
                project_id,
                replica_id,
                snapshot,
                file_scan_inclusions: settings.parent_dir_scan_inclusions.clone(),
                background_snapshot: background_snapshot.clone(),
                updates_tx: Some(background_updates_tx),
                update_observer: None,
                snapshot_subscriptions: Default::default(),
                visible: worktree.visible,
                disconnected: false,
                received_initial_update: false,
            };

            // Apply updates to a separate snapshot in a background task, then
            // send them to a foreground task which updates the model.
            cx.background_spawn(async move {
                while let Some(update) = background_updates_rx.next().await {
                    {
                        let mut lock = background_snapshot.lock();
                        lock.0.apply_remote_update(
                            update.clone(),
                            &settings.parent_dir_scan_inclusions,
                        );
                        lock.1.push(update);
                    }
                    snapshot_updated_tx.send(()).await.ok();
                }
            })
            .detach();

            // On the foreground task, update to the latest snapshot and notify
            // any update observer of all updates that led to that snapshot.
            cx.spawn(async move |this, cx| {
                while (snapshot_updated_rx.recv().await).is_some() {
                    this.update(cx, |this, cx| {
                        let this = this.as_remote_mut().unwrap();

                        // The watch channel delivers an initial signal before
                        // any real updates arrive. Skip these spurious wakeups.
                        if this.background_snapshot.lock().1.is_empty() {
                            return;
                        }

                        let old_root_repo_common_dir = this.snapshot.root_repo_common_dir.clone();
                        let old_root_repo_is_linked_worktree =
                            this.snapshot.root_repo_is_linked_worktree;
                        let mut changed_entries: Vec<(Arc<RelPath>, ProjectEntryId, PathChange)> =
                            Vec::new();
                        {
                            let mut lock = this.background_snapshot.lock();
                            // Replace the snapshot, keeping the previous one around so we can
                            // resolve the paths of removed entries (the new snapshot no longer
                            // contains them, and the wire format only carries their ids).
                            let old_snapshot = mem::replace(&mut this.snapshot, lock.0.clone());
                            for update in lock.1.drain(..) {
                                for entry_id in &update.removed_entries {
                                    let entry_id = ProjectEntryId::from_proto(*entry_id);
                                    if let Some(entry) = old_snapshot.entry_for_id(entry_id) {
                                        changed_entries.push((
                                            entry.path.clone(),
                                            entry_id,
                                            PathChange::Removed,
                                        ));
                                    }
                                }
                                for entry in &update.updated_entries {
                                    // Remote updates don't distinguish creation from
                                    // modification, so report `AddedOrUpdated`.
                                    if let Some(path) =
                                        RelPath::from_unix_str(&entry.path).log_err()
                                    {
                                        changed_entries.push((
                                            path.into(),
                                            ProjectEntryId::from_proto(entry.id),
                                            PathChange::AddedOrUpdated,
                                        ));
                                    }
                                }
                                if let Some(tx) = &this.update_observer {
                                    tx.unbounded_send(update).ok();
                                }
                            }
                        };

                        if !changed_entries.is_empty() {
                            cx.emit(Event::UpdatedEntries(changed_entries.into()));
                        }
                        let is_first_update = !this.received_initial_update;
                        this.received_initial_update = true;
                        if this.snapshot.root_repo_common_dir != old_root_repo_common_dir
                            || this.snapshot.root_repo_is_linked_worktree
                                != old_root_repo_is_linked_worktree
                            || (is_first_update && this.snapshot.root_repo_common_dir.is_none())
                        {
                            cx.emit(Event::UpdatedRootRepoCommonDir {
                                old: old_root_repo_common_dir,
                            });
                        }
                        cx.notify();
                        while let Some((scan_id, _)) = this.snapshot_subscriptions.front() {
                            if this.observed_snapshot(*scan_id) {
                                let (_, tx) = this.snapshot_subscriptions.pop_front().unwrap();
                                let _ = tx.send(());
                            } else {
                                break;
                            }
                        }
                    })?;
                }
                anyhow::Ok(())
            })
            .detach();

            Worktree::Remote(worktree)
        })
    }

    pub fn as_local(&self) -> Option<&LocalWorktree> {
        if let Worktree::Local(worktree) = self {
            Some(worktree)
        } else {
            None
        }
    }

    pub fn as_remote(&self) -> Option<&RemoteWorktree> {
        if let Worktree::Remote(worktree) = self {
            Some(worktree)
        } else {
            None
        }
    }

    pub fn as_local_mut(&mut self) -> Option<&mut LocalWorktree> {
        if let Worktree::Local(worktree) = self {
            Some(worktree)
        } else {
            None
        }
    }

    pub fn as_remote_mut(&mut self) -> Option<&mut RemoteWorktree> {
        if let Worktree::Remote(worktree) = self {
            Some(worktree)
        } else {
            None
        }
    }

    pub fn is_local(&self) -> bool { matches!(self, Worktree::Local(_)) }

    pub fn is_remote(&self) -> bool { !self.is_local() }

    pub fn settings_location(&self, _: &Context<Self>) -> SettingsLocation<'static> {
        SettingsLocation {
            worktree_id: self.id(),
            path: RelPath::empty(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        match self {
            Worktree::Local(worktree) => worktree.snapshot.snapshot.clone(),
            Worktree::Remote(worktree) => worktree.snapshot.clone(),
        }
    }

    pub fn scan_id(&self) -> usize {
        match self {
            Worktree::Local(worktree) => worktree.snapshot.scan_id,
            Worktree::Remote(worktree) => worktree.snapshot.scan_id,
        }
    }

    pub fn metadata_proto(&self) -> proto::WorktreeMetadata {
        proto::WorktreeMetadata {
            id: self.id().to_proto(),
            root_name: self.root_name().as_unix_str().to_owned(),
            visible: self.is_visible(),
            abs_path: self.abs_path().to_string_lossy().into_owned(),
            root_repo_common_dir: self
                .root_repo_common_dir()
                .map(|p| p.to_string_lossy().into_owned()),
            root_repo_is_linked_worktree: self.root_repo_is_linked_worktree(),
        }
    }

    pub fn completed_scan_id(&self) -> usize {
        match self {
            Worktree::Local(worktree) => worktree.snapshot.completed_scan_id,
            Worktree::Remote(worktree) => worktree.snapshot.completed_scan_id,
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Worktree::Local(worktree) => worktree.visible,
            Worktree::Remote(worktree) => worktree.visible,
        }
    }

    pub fn replica_id(&self) -> ReplicaId {
        match self {
            Worktree::Local(_) => ReplicaId::LOCAL,
            Worktree::Remote(worktree) => worktree.replica_id,
        }
    }

    pub fn abs_path(&self) -> Arc<Path> {
        match self {
            Worktree::Local(worktree) => SanitizedPath::cast_arc(worktree.abs_path.clone()),
            Worktree::Remote(worktree) => SanitizedPath::cast_arc(worktree.abs_path.clone()),
        }
    }

    pub fn root_file(&self, cx: &Context<Self>) -> Option<Arc<File>> {
        let entry = self.root_entry()?;
        Some(File::for_entry(entry.clone(), cx.entity()))
    }

    pub fn observe_updates<F, Fut>(&mut self, project_id: u64, cx: &Context<Worktree>, callback: F)
    where
        F: 'static + Send + Fn(proto::UpdateWorktree) -> Fut,
        Fut: 'static + Send + Future<Output = bool>,
    {
        match self {
            Worktree::Local(this) => this.observe_updates(project_id, cx, callback),
            Worktree::Remote(this) => this.observe_updates(project_id, cx, callback),
        }
    }

    pub fn stop_observing_updates(&mut self) {
        match self {
            Worktree::Local(this) => {
                this.update_observer.take();
            }
            Worktree::Remote(this) => {
                this.update_observer.take();
            }
        }
    }

    pub fn wait_for_snapshot(
        &mut self,
        scan_id: usize,
    ) -> impl Future<Output = Result<()>> + use<> {
        match self {
            Worktree::Local(this) => this.wait_for_snapshot(scan_id).boxed(),
            Worktree::Remote(this) => this.wait_for_snapshot(scan_id).boxed(),
        }
    }

    #[cfg(feature = "test-support")]
    pub fn has_update_observer(&self) -> bool {
        match self {
            Worktree::Local(this) => this.update_observer.is_some(),
            Worktree::Remote(this) => this.update_observer.is_some(),
        }
    }

    pub fn load_file(&self, path: &RelPath, cx: &Context<Worktree>) -> Task<Result<LoadedFile>> {
        match self {
            Worktree::Local(this) => this.load_file(path, cx),
            Worktree::Remote(_) => {
                Task::ready(Err(anyhow!("remote worktrees can't yet load files")))
            }
        }
    }

    pub fn load_binary_file(
        &self,
        path: &RelPath,
        cx: &Context<Worktree>,
    ) -> Task<Result<LoadedBinaryFile>> {
        match self {
            Worktree::Local(this) => this.load_binary_file(path, cx),
            Worktree::Remote(_) => {
                Task::ready(Err(anyhow!("remote worktrees can't yet load binary files")))
            }
        }
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
        match self {
            Worktree::Local(this) => {
                this.write_file(path, text, line_ending, encoding, has_bom, cx)
            }
            Worktree::Remote(_) => {
                Task::ready(Err(anyhow!("remote worktree can't yet write files")))
            }
        }
    }

    pub fn create_entry(
        &mut self,
        path: Arc<RelPath>,
        is_directory: bool,
        content: Option<Vec<u8>>,
        cx: &Context<Worktree>,
    ) -> Task<Result<CreatedEntry>> {
        let worktree_id = self.id();
        match self {
            Worktree::Local(this) => this.create_entry(path, is_directory, content, cx),
            Worktree::Remote(this) => {
                let project_id = this.project_id;
                let request = this.client.request(proto::CreateProjectEntry {
                    worktree_id: worktree_id.to_proto(),
                    project_id,
                    path: path.as_ref().as_unix_str().to_owned(),
                    content,
                    is_directory,
                });
                cx.spawn(async move |this, cx| {
                    let response = request.await?;
                    match response.entry {
                        Some(entry) => this
                            .update(cx, |worktree, cx| {
                                worktree.as_remote_mut().unwrap().insert_entry(
                                    entry,
                                    response.worktree_scan_id as usize,
                                    cx,
                                )
                            })?
                            .await
                            .map(CreatedEntry::Included),
                        None => {
                            let abs_path =
                                this.read_with(cx, |worktree, _| worktree.absolutize(&path))?;
                            Ok(CreatedEntry::Excluded { abs_path })
                        }
                    }
                })
            }
        }
    }

    pub fn trash_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &mut Context<Worktree>,
    ) -> Option<Task<Result<TrashId>>> {
        let entry = match self {
            Worktree::Local(this) => this.entry_for_id(entry_id),
            Worktree::Remote(this) => this.entry_for_id(entry_id),
        }?
        .clone();

        let task = match self {
            Worktree::Local(this) => this.trash_entry(entry.clone(), cx),
            Worktree::Remote(this) => this.trash_entry(entry_id, cx),
        };

        let mut ids = vec![entry_id];
        self.get_children_ids_recursive(&entry.path, &mut ids);

        for id in ids {
            cx.emit(Event::DeletedEntry(id));
        }
        Some(task)
    }

    pub fn delete_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &mut Context<Worktree>,
    ) -> Option<Task<Result<()>>> {
        let entry = match self {
            Worktree::Local(this) => this.entry_for_id(entry_id),
            Worktree::Remote(this) => this.entry_for_id(entry_id),
        }?
        .clone();

        let task = match self {
            Worktree::Local(this) => this.delete_entry(entry.clone(), cx),
            Worktree::Remote(this) => this.delete_entry(entry_id, cx),
        };

        let mut ids = vec![entry_id];
        let path = entry.path;

        self.get_children_ids_recursive(&path, &mut ids);

        for id in ids {
            cx.emit(Event::DeletedEntry(id));
        }
        Some(task)
    }

    pub fn restore_entry(
        &mut self,
        trash_id: TrashId,
        cx: &mut Context<'_, Worktree>,
    ) -> Task<Result<Entry>> {
        match self {
            Worktree::Local(this) => this.restore_entry(trash_id, cx),
            Worktree::Remote(this) => this.restore_entry(trash_id, cx),
        }
    }

    fn get_children_ids_recursive(&self, path: &RelPath, ids: &mut Vec<ProjectEntryId>) {
        let children_iter = self.child_entries(path);
        for child in children_iter {
            ids.push(child.id);
            self.get_children_ids_recursive(&child.path, ids);
        }
    }

    pub fn copy_external_entries(
        &mut self,
        target_directory: Arc<RelPath>,
        paths: Vec<Arc<Path>>,
        fs: Arc<dyn Fs>,
        cx: &Context<Worktree>,
    ) -> Task<Result<Vec<ProjectEntryId>>> {
        match self {
            Worktree::Local(this) => this.copy_external_entries(target_directory, paths, cx),
            Worktree::Remote(this) => this.copy_external_entries(target_directory, paths, fs, cx),
        }
    }

    pub fn expand_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Option<Task<Result<()>>> {
        match self {
            Worktree::Local(this) => this.expand_entry(entry_id, cx),
            Worktree::Remote(this) => {
                let response = this.client.request(proto::ExpandProjectEntry {
                    project_id: this.project_id,
                    entry_id: entry_id.to_proto(),
                });
                Some(cx.spawn(async move |this, cx| {
                    let response = response.await?;
                    this.update(cx, |this, _| {
                        this.as_remote_mut()
                            .unwrap()
                            .wait_for_snapshot(response.worktree_scan_id as usize)
                    })?
                    .await?;
                    Ok(())
                }))
            }
        }
    }

    pub fn expand_all_for_entry(
        &mut self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Option<Task<Result<()>>> {
        match self {
            Worktree::Local(this) => this.expand_all_for_entry(entry_id, cx),
            Worktree::Remote(this) => {
                let response = this.client.request(proto::ExpandAllForProjectEntry {
                    project_id: this.project_id,
                    entry_id: entry_id.to_proto(),
                });
                Some(cx.spawn(async move |this, cx| {
                    let response = response.await?;
                    this.update(cx, |this, _| {
                        this.as_remote_mut()
                            .unwrap()
                            .wait_for_snapshot(response.worktree_scan_id as usize)
                    })?
                    .await?;
                    Ok(())
                }))
            }
        }
    }

    pub async fn handle_create_entry(
        this: Entity<Self>,
        request: proto::CreateProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::ProjectEntryResponse> {
        let (scan_id, entry) = this.update(&mut cx, |this, cx| {
            anyhow::Ok((
                this.scan_id(),
                this.create_entry(
                    RelPath::from_unix_str(&request.path)
                        .with_context(|| {
                            format!("received invalid relative path {:?}", request.path)
                        })?
                        .into(),
                    request.is_directory,
                    request.content,
                    cx,
                ),
            ))
        })?;
        Ok(proto::ProjectEntryResponse {
            entry: match &entry.await? {
                CreatedEntry::Included(entry) => Some(entry.into()),
                CreatedEntry::Excluded { .. } => None,
            },
            worktree_scan_id: scan_id as u64,
        })
    }

    pub async fn handle_trash_entry(
        this: Entity<Self>,
        request: proto::TrashProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::TrashProjectEntryResponse> {
        let (scan_id, task) = this.update(&mut cx, |this, cx| {
            (
                this.scan_id(),
                this.trash_entry(ProjectEntryId::from_proto(request.entry_id), cx),
            )
        });
        let trash_id = task
            .ok_or_else(|| anyhow::anyhow!("invalid entry"))?
            .await?;

        Ok(proto::TrashProjectEntryResponse {
            trash_id: trash_id.to_proto(),
            worktree_scan_id: scan_id as u64,
        })
    }

    pub async fn handle_delete_entry(
        this: Entity<Self>,
        request: proto::DeleteProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::ProjectEntryResponse> {
        let (scan_id, task) = this.update(&mut cx, |this, cx| {
            // While the `use_trash` field is deprecated but not removed, we
            // still need to support either trashing or deleting the file.
            // Otherwise, if an older client sends the `DeleteProjectEntry {
            // use_trash: true }` rather than the newer `TrashProjectEntry`, and
            // the flag was ignored, we'd permanently delete a file that was
            // actually meant to be trashed.
            #[allow(deprecated)]
            let task = if request.use_trash {
                this.trash_entry(ProjectEntryId::from_proto(request.entry_id), cx)
                    .map(|task| cx.background_spawn(async move { task.await.map(|_| ()) }))
            } else {
                this.delete_entry(ProjectEntryId::from_proto(request.entry_id), cx)
            };

            (this.scan_id(), task)
        });
        task.ok_or_else(|| anyhow::anyhow!("invalid entry"))?
            .await?;
        Ok(proto::ProjectEntryResponse {
            entry: None,
            worktree_scan_id: scan_id as u64,
        })
    }

    pub async fn handle_restore_entry(
        this: Entity<Self>,
        request: proto::RestoreProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::RestoreProjectEntryResponse> {
        let (scan_id, task) = this.update(&mut cx, |this, cx| {
            (
                this.scan_id(),
                this.restore_entry(TrashId::from_proto(request.trash_id), cx),
            )
        });

        let entry = task.await?;

        Ok(proto::RestoreProjectEntryResponse {
            entry: Some(proto::Entry::from(&entry)),
            worktree_scan_id: scan_id as u64,
        })
    }

    pub async fn handle_expand_entry(
        this: Entity<Self>,
        request: proto::ExpandProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::ExpandProjectEntryResponse> {
        let task = this.update(&mut cx, |this, cx| {
            this.expand_entry(ProjectEntryId::from_proto(request.entry_id), cx)
        });
        task.ok_or_else(|| anyhow::anyhow!("no such entry"))?
            .await?;
        let scan_id = this.read_with(&cx, |this, _| this.scan_id());
        Ok(proto::ExpandProjectEntryResponse {
            worktree_scan_id: scan_id as u64,
        })
    }

    pub async fn handle_expand_all_for_entry(
        this: Entity<Self>,
        request: proto::ExpandAllForProjectEntry,
        mut cx: AsyncApp,
    ) -> Result<proto::ExpandAllForProjectEntryResponse> {
        let task = this.update(&mut cx, |this, cx| {
            this.expand_all_for_entry(ProjectEntryId::from_proto(request.entry_id), cx)
        });
        task.ok_or_else(|| anyhow::anyhow!("no such entry"))?
            .await?;
        let scan_id = this.read_with(&cx, |this, _| this.scan_id());
        Ok(proto::ExpandAllForProjectEntryResponse {
            worktree_scan_id: scan_id as u64,
        })
    }

    pub fn is_single_file(&self) -> bool { self.root_dir().is_none() }

    /// For visible worktrees, returns the path with the worktree name as the first component.
    /// Otherwise, returns an absolute path.
    pub fn full_path(&self, worktree_relative_path: &RelPath) -> PathBuf {
        if self.is_visible() {
            self.root_name()
                .join(worktree_relative_path)
                .display(self.path_style)
                .to_string()
                .into()
        } else {
            let full_path = self.abs_path();
            let mut full_path_string = if self.is_local()
                && let Ok(stripped) = full_path.strip_prefix(home_dir())
            {
                self.path_style
                    .join("~", &*stripped.to_string_lossy())
                    .unwrap()
            } else {
                full_path.to_string_lossy().into_owned()
            };

            if worktree_relative_path.components().next().is_some() {
                full_path_string.push_str(self.path_style.primary_separator());
                full_path_string.push_str(&worktree_relative_path.display(self.path_style));
            }

            full_path_string.into()
        }
    }
}

impl Deref for Worktree {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target {
        match self {
            Worktree::Local(worktree) => &worktree.snapshot,
            Worktree::Remote(worktree) => &worktree.snapshot,
        }
    }
}
