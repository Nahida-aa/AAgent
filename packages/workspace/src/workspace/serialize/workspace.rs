use super::*;
use crate::workspace::serialize::pane::serialize_pane_handle;

pub(crate) enum WorkspaceLocation {
    // Valid local paths or SSH project to serialize
    Location(SerializedWorkspaceLocation, PathList),
    // No valid location found to serialize
    None,
}

impl Workspace {
    //
    pub(crate) fn workspace_location(&self, cx: &App) -> WorkspaceLocation {
        let paths = PathList::new(&self.root_paths(cx));
        if let Some(connection) = self.project.read(cx).remote_connection_options(cx) {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Remote(connection), paths)
        } else if self.project.read(cx).is_local() {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Local, paths)
        } else {
            WorkspaceLocation::None
        }
    }
    //
    pub(crate) fn remove_from_session(&mut self, window: &mut Window, cx: &mut App) -> Task<()> {
        self.session_id.take();
        self.serialize_workspace_internal(window, cx)
    }
    //
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
    //
    pub(crate) fn serialize_workspace_internal(&self, window: &mut Window, cx: &mut App) -> Task<()> {
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
    //
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
}
