use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    pub(crate) fn serialize_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._schedule_serialize_workspace.is_none() {
            self._schedule_serialize_workspace =
                Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(SERIALIZATION_THROTTLE_TIME)
                        .await;
                    this.update_in(cx, |this, window, cx| {
                        this._serialize_workspace_task =
                            Some(this.serialize_workspace_internal(window, cx));
                        this._schedule_serialize_workspace.take();
                    })
                    .log_err();
                }));
        }
    }

    fn serialize_workspace_internal(&self, window: &mut Window, cx: &mut App) -> Task<()> {
        let Some(database_id) = self.database_id() else {
            return Task::ready(());
        };

        fn build_serialized_pane_group(
            pane_group: &Member,
            window: &mut Window,
            cx: &mut App,
        ) -> SerializedPaneGroup {
            match pane_group {
                Member::Axis(PaneAxis {
                    axis,
                    members,
                    flexes,
                    bounding_boxes: _,
                }) => SerializedPaneGroup::Group {
                    axis: SerializedAxis(*axis),
                    children: members
                        .iter()
                        .map(|member| build_serialized_pane_group(member, window, cx))
                        .collect::<Vec<_>>(),
                    flexes: Some(flexes.lock().clone()),
                },
                Member::Pane(pane_handle) => {
                    SerializedPaneGroup::Pane(serialize_pane_handle(pane_handle, window, cx))
                }
            }
        }

        fn build_serialized_docks(
            this: &Workspace,
            window: &mut Window,
            cx: &mut App,
        ) -> DockStructure {
            this.capture_dock_state(window, cx)
        }

        match self.workspace_location(cx) {
            WorkspaceLocation::Location(location, paths) => {
                let bookmarks = self.project.update(cx, |project, cx| {
                    project
                        .bookmark_store()
                        .read(cx)
                        .all_serialized_bookmarks(cx)
                });

                let breakpoints = self.project.update(cx, |project, cx| {
                    project
                        .breakpoint_store()
                        .read(cx)
                        .all_source_breakpoints(cx)
                });
                let user_toolchains = self
                    .project
                    .read(cx)
                    .user_toolchains(cx)
                    .unwrap_or_default();

                let center_group = build_serialized_pane_group(&self.center.root, window, cx);
                let docks = build_serialized_docks(self, window, cx);
                let default_docks = (paths.is_empty()
                    && location == SerializedWorkspaceLocation::Local)
                    .then(|| docks.clone());
                let window_bounds = Some(SerializedWindowBounds(window.window_bounds()));
                let identity_paths_hint = self.project_group_key(cx).path_list().clone();
                let recent_navigation_history = self.persisted_recent_navigation_history.clone();

                let serialized_workspace = SerializedWorkspace {
                    id: database_id,
                    location,
                    paths,
                    identity_paths: Some(identity_paths_hint),
                    center_group,
                    window_bounds,
                    display: Default::default(),
                    docks,
                    centered_layout: self.centered_layout,
                    session_id: self.session_id.clone(),
                    bookmarks,
                    breakpoints,
                    window_id: Some(window.window_handle().window_id().as_u64()),
                    user_toolchains,
                    recent_navigation_history,
                };

                let db = WorkspaceDb::global(cx);
                let kvp = db::kvp::KeyValueStore::global(cx);
                cx.background_spawn(async move {
                    if let Some(docks) = default_docks {
                        persistence::write_default_dock_state(&kvp, docks)
                            .await
                            .log_err();
                    }
                    db.save_workspace(serialized_workspace).await;
                })
            }
            WorkspaceLocation::None => {
                // Save dock state for empty non-local workspaces
                let docks = build_serialized_docks(self, window, cx);
                let kvp = db::kvp::KeyValueStore::global(cx);
                cx.background_spawn(async move {
                    persistence::write_default_dock_state(&kvp, docks)
                        .await
                        .log_err();
                })
            }
        }
    }

    fn workspace_location(&self, cx: &App) -> WorkspaceLocation {
        let paths = PathList::new(&self.root_paths(cx));
        if let Some(connection) = self.project.read(cx).remote_connection_options(cx) {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Remote(connection), paths)
        } else if self.project.read(cx).is_local() {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Local, paths)
        } else {
            WorkspaceLocation::None
        }
    }

    pub(super) async fn serialize_items(
        this: &WeakEntity<Self>,
        items_rx: UnboundedReceiver<Box<dyn SerializableItemHandle>>,
        cx: &mut AsyncWindowContext,
    ) -> Result<()> {
        const CHUNK_SIZE: usize = 200;

        let mut serializable_items = items_rx.ready_chunks(CHUNK_SIZE);

        while let Some(items_received) = serializable_items.next().await {
            let unique_items =
                items_received
                    .into_iter()
                    .fold(HashMap::default(), |mut acc, item| {
                        acc.entry(item.item_id()).or_insert(item);
                        acc
                    });

            // We use into_iter() here so that the references to the items are moved into
            // the tasks and not kept alive while we're sleeping.
            for (_, item) in unique_items.into_iter() {
                if let Ok(Some(task)) =
                    this.update(cx, |workspace, cx| item.serialize(workspace, false, cx))
                {
                    cx.background_spawn(async move { task.await.log_err() })
                        .detach();
                }
            }

            cx.background_executor()
                .timer(SERIALIZATION_THROTTLE_TIME)
                .await;
        }

        Ok(())
    }

    pub(crate) fn enqueue_item_serialization(
        &mut self,
        item: Box<dyn SerializableItemHandle>,
    ) -> Result<()> {
        self.serializable_items_tx
            .unbounded_send(item)
            .map_err(|err| anyhow!("failed to send serializable item over channel: {err}"))
    }

    pub(crate) fn load_workspace(
        serialized_workspace: SerializedWorkspace,
        paths_to_open: Vec<Option<ProjectPath>>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Task<Result<Vec<Option<Box<dyn ItemHandle>>>>> {
        cx.spawn_in(window, async move |workspace, cx| {
            let recent_navigation_history = serialized_workspace.recent_navigation_history.clone();
            workspace.update(cx, |workspace, _| {
                workspace.persisted_recent_navigation_history = recent_navigation_history;
                workspace.restoring_workspace = true;
            })?;
            let project = workspace.read_with(cx, |workspace, _| workspace.project().clone())?;

            let mut center_group = None;
            let mut center_items = None;

            // Traverse the splits tree and add to things
            if let Some((group, active_pane, items)) = serialized_workspace
                .center_group
                .deserialize(&project, serialized_workspace.id, workspace.clone(), cx)
                .await
            {
                center_items = Some(items);
                center_group = Some((group, active_pane))
            }

            let mut items_by_project_path = HashMap::default();
            let mut item_ids_by_kind = HashMap::default();
            let mut all_deserialized_items = Vec::default();
            cx.update(|_, cx| {
                for item in center_items.unwrap_or_default().into_iter().flatten() {
                    if let Some(serializable_item_handle) = item.to_serializable_item_handle(cx) {
                        item_ids_by_kind
                            .entry(serializable_item_handle.serialized_item_kind())
                            .or_insert(Vec::new())
                            .push(item.item_id().as_u64() as ItemId);
                    }

                    if let Some(project_path) = item.project_path(cx) {
                        items_by_project_path.insert(project_path, item.clone());
                    }
                    all_deserialized_items.push(item);
                }
            })?;

            let opened_items = paths_to_open
                .into_iter()
                .map(|path_to_open| {
                    path_to_open
                        .and_then(|path_to_open| items_by_project_path.remove(&path_to_open))
                })
                .collect::<Vec<_>>();

            // Remove old panes from workspace panes list
            workspace.update_in(cx, |workspace, window, cx| {
                if let Some((center_group, active_pane)) = center_group {
                    workspace.remove_panes(workspace.center.root.clone(), window, cx);

                    // Swap workspace center group
                    workspace.center = PaneGroup::with_root(center_group);
                    workspace.center.set_is_center(true);
                    workspace.center.mark_positions(cx);

                    if let Some(active_pane) = active_pane {
                        workspace.set_active_pane(&active_pane, window, cx);
                        cx.focus_self(window);
                    } else {
                        workspace.set_active_pane(&workspace.center.first_pane(), window, cx);
                    }
                }

                let docks = serialized_workspace.docks;

                for (dock, serialized_dock) in [
                    (&mut workspace.right_dock, docks.right),
                    (&mut workspace.left_dock, docks.left),
                    (&mut workspace.bottom_dock, docks.bottom),
                ]
                .iter_mut()
                {
                    dock.update(cx, |dock, cx| {
                        dock.restore_serialized_state(serialized_dock.clone(), window, cx);
                    });
                }

                workspace.restoring_workspace = false;
                cx.notify();
            })?;

            project
                .update(cx, |project, cx| {
                    project.bookmark_store().update(cx, |bookmark_store, cx| {
                        bookmark_store.load_serialized_bookmarks(serialized_workspace.bookmarks, cx)
                    })
                })
                .await
                .log_err();

            let _ = project
                .update(cx, |project, cx| {
                    project
                        .breakpoint_store()
                        .update(cx, |breakpoint_store, cx| {
                            breakpoint_store
                                .with_serialized_breakpoints(serialized_workspace.breakpoints, cx)
                        })
                })
                .await;

            // Clean up all the items that have _not_ been loaded. Our ItemIds aren't stable. That means
            // after loading the items, we might have different items and in order to avoid
            // the database filling up, we delete items that haven't been loaded now.
            //
            // The items that have been loaded, have been saved after they've been added to the workspace.
            let clean_up_tasks = workspace.update_in(cx, |_, window, cx| {
                item_ids_by_kind
                    .into_iter()
                    .map(|(item_kind, loaded_items)| {
                        SerializableItemRegistry::cleanup(
                            item_kind,
                            serialized_workspace.id,
                            loaded_items,
                            window,
                            cx,
                        )
                        .log_err()
                    })
                    .collect::<Vec<_>>()
            })?;

            futures::future::join_all(clean_up_tasks).await;

            workspace
                .update_in(cx, |workspace, window, cx| {
                    // Serialize ourself to make sure our timestamps and any pane / item changes are replicated
                    workspace.serialize_workspace_internal(window, cx).detach();

                    // Ensure that we mark the window as edited if we did load dirty items
                    workspace.update_window_edited(window, cx);
                })
                .ok();

            Ok(opened_items)
        })
    }

    /// Bypass the serialization throttles and write workspace and item state
    /// to the DB immediately. Returns a task the caller can await to ensure the
    /// writes complete before the process exits.
    pub fn flush_serialization(&mut self, window: &mut Window, cx: &mut App) -> Task<()> {
        self._schedule_serialize_workspace.take();
        self._serialize_workspace_task.take();
        self.bounds_save_task_queued.take();

        let serializable_items = self
            .panes
            .iter()
            .flat_map(|pane| pane.read(cx).items())
            .filter_map(|item| item.to_serializable_item_handle(cx))
            .fold(HashMap::default(), |mut items, item| {
                items.entry(item.item_id()).or_insert(item);
                items
            });
        let item_tasks = serializable_items
            .into_values()
            .filter_map(|item| {
                let item_id = item.item_id();
                let task = item.serialize(self, false, cx)?;
                Some(async move {
                    task.await
                        .with_context(|| format!("flushing serialization of item {item_id:?}"))
                })
            })
            .collect::<Vec<_>>();
        let bounds_task = self.save_window_bounds(window, cx);
        let serialize_task = self.serialize_workspace_internal(window, cx);
        cx.background_spawn(async move {
            bounds_task.await;
            serialize_task.await;
            for result in futures::future::join_all(item_tasks).await {
                result.log_err();
            }
        })
    }

    fn save_window_bounds(&self, window: &mut Window, cx: &mut App) -> Task<()> {
        let Some(display) = window.display(cx) else {
            return Task::ready(());
        };
        let Ok(display_uuid) = display.uuid() else {
            return Task::ready(());
        };

        let window_bounds = window.inner_window_bounds();
        let database_id = self.database_id;
        let has_paths = !self.root_paths(cx).is_empty();
        let db = WorkspaceDb::global(cx);
        let kvp = db::kvp::KeyValueStore::global(cx);
        let native_window_state = if database_id.is_some() {
            window.native_window_state()
        } else {
            None
        };

        cx.background_executor().spawn(async move {
            if !has_paths {
                persistence::write_default_window_bounds(&kvp, window_bounds, display_uuid)
                    .await
                    .log_err();
            }
            if let Some(database_id) = database_id {
                db.set_window_open_status(
                    database_id,
                    SerializedWindowBounds(window_bounds),
                    display_uuid,
                    native_window_state,
                )
                .await
                .log_err();
            } else {
                persistence::write_default_window_bounds(&kvp, window_bounds, display_uuid)
                    .await
                    .log_err();
            }
        })
    }
}

fn serialize_pane_handle(
    pane_handle: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) -> SerializedPane {
    let (items, active, pinned_count) = {
        let pane = pane_handle.read(cx);
        let active_item_id = pane.active_item().map(|item| item.item_id());
        // Pinned tabs are the leading tabs of a pane, so the pinned count has to
        // shrink along with every pinned item that is dropped here. Otherwise a
        // tab that was not pinned would take the dropped item's slot and come
        // back pinned on the next restore.
        let pinned_region = 0..pane.pinned_count();
        let mut pinned_count = pane.pinned_count();
        let items = pane
            .items()
            .enumerate()
            .filter_map(|(index, handle)| {
                let Some(handle) = handle.to_serializable_item_handle(cx) else {
                    if pinned_region.contains(&index) {
                        pinned_count -= 1;
                    }
                    return None;
                };

                Some(SerializedItem {
                    kind: Arc::from(handle.serialized_item_kind()),
                    item_id: handle.item_id().as_u64(),
                    active: Some(handle.item_id()) == active_item_id,
                    preview: pane.is_active_preview_item(handle.item_id()),
                })
            })
            .collect::<Vec<_>>();

        (items, pane.has_focus(window, cx), pinned_count)
    };

    SerializedPane::new(items, active, pinned_count)
}

pub async fn flush_windows_serialization(
    workspace_windows: &[WindowHandle<MultiWorkspace>],
    cx: &mut AsyncApp,
) {
    let flush_tasks = collect_flush_tasks(workspace_windows, cx);
    futures::future::join_all(flush_tasks).await;
}

fn flush_windows_serialization_on_quit(cx: &mut App) -> impl Future<Output = ()> + use<> {
    let workspace_windows = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<MultiWorkspace>())
        .collect::<Vec<_>>();
    let flush_tasks = collect_flush_tasks(&workspace_windows, cx);
    async move {
        futures::future::join_all(flush_tasks).await;
    }
}

fn collect_flush_tasks(
    workspace_windows: &[WindowHandle<MultiWorkspace>],
    cx: &mut impl AppContext,
) -> Vec<Task<()>> {
    let mut flush_tasks = Vec::new();
    for window in workspace_windows {
        window
            .update(cx, |multi_workspace, window, cx| {
                flush_tasks.extend(multi_workspace.flush_pending_serialization(window, cx));
            })
            .with_context(|| {
                format!(
                    "flushing pending serialization for window {:?}",
                    window.window_id()
                )
            })
            .log_err();
    }
    flush_tasks
}

pub fn reload(cx: &mut App) {
    let should_confirm = WorkspaceSettings::get_global(cx).confirm_quit;
    let mut workspace_windows = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<MultiWorkspace>())
        .collect::<Vec<_>>();

    // If multiple windows have unsaved changes, and need a save prompt,
    // prompt in the active window before switching to a different window.
    workspace_windows.sort_by_key(|window| window.is_active(cx) == Some(false));

    let mut prompt = None;
    if let (true, Some(window)) = (should_confirm, workspace_windows.first()) {
        prompt = window
            .update(cx, |_, window, cx| {
                window.prompt(
                    PromptLevel::Info,
                    "Are you sure you want to restart?",
                    None,
                    &["Restart", "Cancel"],
                    cx,
                )
            })
            .ok();
    }

    cx.spawn(async move |cx| {
        if let Some(prompt) = prompt {
            let answer = prompt.await?;
            if answer != 0 {
                return anyhow::Ok(());
            }
        }

        if !prepare_windows_to_quit(&workspace_windows, cx).await {
            return anyhow::Ok(());
        }
        cx.update(|cx| cx.restart());
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}
