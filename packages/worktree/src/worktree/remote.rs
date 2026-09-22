use super::*;
use rpc::proto;

pub struct RemoteWorktree {
    pub(crate) snapshot: Snapshot,
    pub(crate) background_snapshot: Arc<Mutex<(Snapshot, Vec<proto::UpdateWorktree>)>>,
    pub(crate) project_id: u64,
    pub(crate) client: AnyProtoClient,
    pub(crate) file_scan_inclusions: PathMatcher,
    pub(crate) updates_tx: Option<UnboundedSender<proto::UpdateWorktree>>,
    pub(crate) update_observer: Option<mpsc::UnboundedSender<proto::UpdateWorktree>>,
    pub(crate) snapshot_subscriptions: VecDeque<(usize, oneshot::Sender<()>)>,
    pub(crate) replica_id: ReplicaId,
    pub(crate) visible: bool,
    pub(crate) disconnected: bool,
    pub(crate) received_initial_update: bool,
}
impl RemoteWorktree {
    pub fn project_id(&self) -> u64 { self.project_id }

    pub fn client(&self) -> AnyProtoClient { self.client.clone() }

    pub fn disconnected_from_host(&mut self) {
        self.updates_tx.take();
        self.snapshot_subscriptions.clear();
        self.disconnected = true;
    }

    pub fn update_from_remote(&self, update: proto::UpdateWorktree) {
        if let Some(updates_tx) = &self.updates_tx {
            updates_tx
                .unbounded_send(update)
                .expect("consumer runs to completion");
        }
    }

    pub(crate) fn observe_updates<F, Fut>(
        &mut self,
        project_id: u64,
        cx: &Context<Worktree>,
        callback: F,
    ) where
        F: 'static + Send + Fn(proto::UpdateWorktree) -> Fut,
        Fut: 'static + Send + Future<Output = bool>,
    {
        let (tx, mut rx) = mpsc::unbounded();
        let initial_update = self
            .snapshot
            .build_initial_update(project_id, self.id().to_proto());
        self.update_observer = Some(tx);
        cx.spawn(async move |this, cx| {
            let mut update = initial_update;
            'outer: loop {
                // SSH projects use a special project ID of 0, and we need to
                // remap it to the correct one here.
                update.project_id = project_id;

                for chunk in split_worktree_update(update) {
                    if !callback(chunk).await {
                        break 'outer;
                    }
                }

                if let Some(next_update) = rx.next().await {
                    update = next_update;
                } else {
                    break;
                }
            }
            this.update(cx, |this, _| {
                let this = this.as_remote_mut().unwrap();
                this.update_observer.take();
            })
        })
        .detach();
    }

    pub(crate) fn observed_snapshot(&self, scan_id: usize) -> bool {
        self.completed_scan_id >= scan_id
    }

    pub fn wait_for_snapshot(
        &mut self,
        scan_id: usize,
    ) -> impl Future<Output = Result<()>> + use<> {
        let (tx, rx) = oneshot::channel();
        if self.observed_snapshot(scan_id) {
            let _ = tx.send(());
        } else if self.disconnected {
            drop(tx);
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

    pub fn insert_entry(
        &mut self,
        entry: proto::Entry,
        scan_id: usize,
        cx: &Context<Worktree>,
    ) -> Task<Result<Entry>> {
        let wait_for_snapshot = self.wait_for_snapshot(scan_id);
        cx.spawn(async move |this, cx| {
            wait_for_snapshot.await?;
            this.update(cx, |worktree, _| {
                let worktree = worktree.as_remote_mut().unwrap();
                let snapshot = &mut worktree.background_snapshot.lock().0;
                let entry = snapshot.insert_entry(entry, &worktree.file_scan_inclusions);
                worktree.snapshot = snapshot.clone();
                entry
            })?
        })
    }

    pub(crate) fn trash_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Task<Result<TrashId>> {
        let response = self.client.request(proto::TrashProjectEntry {
            project_id: self.project_id,
            entry_id: entry_id.to_proto(),
        });

        cx.spawn(async move |this, cx| {
            let response = response.await?;
            let scan_id = response.worktree_scan_id as usize;
            let trash_id = response.trash_id;

            this.update(cx, move |this, _| {
                this.as_remote_mut().unwrap().wait_for_snapshot(scan_id)
            })?
            .await?;

            this.update(cx, |this, _| {
                let this = this.as_remote_mut().unwrap();
                let snapshot = &mut this.background_snapshot.lock().0;
                snapshot.delete_entry(entry_id);
                this.snapshot = snapshot.clone();
            })?;

            Ok(TrashId::from_proto(trash_id))
        })
    }

    pub(crate) fn delete_entry(
        &self,
        entry_id: ProjectEntryId,
        cx: &Context<Worktree>,
    ) -> Task<Result<()>> {
        let response = self.client.request(proto::DeleteProjectEntry {
            project_id: self.project_id,
            entry_id: entry_id.to_proto(),
            // The `use_trash` field is being deprecated but it's still required
            // in the message, hence the `#[allow(deprecated)]` attribute.
            #[allow(deprecated)]
            use_trash: false,
        });

        cx.spawn(async move |this, cx| {
            let response = response.await?;
            let scan_id = response.worktree_scan_id as usize;

            this.update(cx, move |this, _| {
                this.as_remote_mut().unwrap().wait_for_snapshot(scan_id)
            })?
            .await?;

            this.update(cx, |this, _| {
                let this = this.as_remote_mut().unwrap();
                let snapshot = &mut this.background_snapshot.lock().0;
                snapshot.delete_entry(entry_id);
                this.snapshot = snapshot.clone();
            })
        })
    }

    pub(crate) fn restore_entry(
        &mut self,
        trash_id: TrashId,
        cx: &Context<Worktree>,
    ) -> Task<Result<Entry>> {
        let project_id = self.project_id();
        let worktree_id = self.id().to_proto();
        let trash_id = trash_id.to_proto();

        let request = self.client.request(proto::RestoreProjectEntry {
            project_id,
            worktree_id,
            trash_id,
        });

        cx.spawn(async move |this, cx| {
            let response = request.await?;
            let scan_id = response.worktree_scan_id as usize;
            let proto_entry = response.entry.context("Missing entry in in response")?;

            this.update(cx, move |worktree, cx| {
                worktree
                    .as_remote_mut()
                    .unwrap()
                    .insert_entry(proto_entry, scan_id, cx)
            })?
            .await
        })
    }

    pub(crate) fn copy_external_entries(
        &self,
        target_directory: Arc<RelPath>,
        paths_to_copy: Vec<Arc<Path>>,
        local_fs: Arc<dyn Fs>,
        cx: &Context<Worktree>,
    ) -> Task<anyhow::Result<Vec<ProjectEntryId>>> {
        let client = self.client.clone();
        let worktree_id = self.id().to_proto();
        let project_id = self.project_id;

        cx.background_spawn(async move {
            let mut requests = Vec::new();
            for root_path_to_copy in paths_to_copy {
                let Some(filename) = root_path_to_copy
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|filename| RelPath::from_unix_str(filename).ok())
                else {
                    continue;
                };
                for (abs_path, is_directory) in
                    read_dir_items(local_fs.as_ref(), &root_path_to_copy).await?
                {
                    let Some(relative_path) = abs_path
                        .strip_prefix(&root_path_to_copy)
                        .map_err(|e| anyhow::Error::from(e))
                        .and_then(|relative_path| RelPath::new(relative_path, PathStyle::local()))
                        .log_err()
                    else {
                        continue;
                    };
                    let content = if is_directory {
                        None
                    } else {
                        Some(local_fs.load_bytes(&abs_path).await?)
                    };

                    let mut target_path = target_directory.join(filename);
                    if relative_path.file_name().is_some() {
                        target_path = target_path.join(&relative_path);
                    }

                    requests.push(proto::CreateProjectEntry {
                        project_id,
                        worktree_id,
                        path: target_path.as_unix_str().to_owned(),
                        is_directory,
                        content,
                    });
                }
            }
            requests.sort_unstable_by(|a, b| a.path.cmp(&b.path));
            requests.dedup();

            let mut copied_entry_ids = Vec::new();
            for request in requests {
                let response = client.request(request).await?;
                copied_entry_ids.extend(response.entry.map(|e| ProjectEntryId::from_proto(e.id)));
            }

            Ok(copied_entry_ids)
        })
    }
}

impl Deref for RemoteWorktree {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target { &self.snapshot }
}
