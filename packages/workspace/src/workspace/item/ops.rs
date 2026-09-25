// /home/aa/repos/ai_ls/AAgent/packages/workspace/src/workspace/item/ops.rs
impl Workspace {
    // 打开
    // pub fn open_resolved_path
    pub fn open_resolved_path(
        &mut self,
        path: ResolvedPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        match path {
            ResolvedPath::ProjectPath { project_path, .. } => {
                self.open_path(project_path, None, true, window, cx)
            }
            ResolvedPath::AbsPath { path, .. } => self.open_abs_path(
                PathBuf::from(path),
                OpenOptions {
                    visible: Some(OpenVisible::None),
                    ..Default::default()
                },
                window,
                cx,
            ),
        }
    }
    // pub fn open_abs_path
    pub fn open_abs_path(
        &mut self,
        abs_path: PathBuf,
        options: OpenOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        cx.spawn_in(window, async move |workspace, cx| {
            let open_paths_task_result = workspace
                .update_in(cx, |workspace, window, cx| {
                    workspace.open_paths(vec![abs_path.clone()], options, None, window, cx)
                })
                .with_context(|| format!("open abs path {abs_path:?} task spawn"))?
                .await;
            anyhow::ensure!(
                open_paths_task_result.len() == 1,
                "open abs path {abs_path:?} task returned incorrect number of results"
            );
            match open_paths_task_result
                .into_iter()
                .next()
                .expect("ensured single task result")
            {
                Some(open_result) => {
                    open_result.with_context(|| format!("open abs path {abs_path:?} task join"))
                }
                None => anyhow::bail!("open abs path {abs_path:?} task returned None"),
            }
        })
    }
    // pub fn split_abs_path
    pub fn split_abs_path(
        &mut self,
        abs_path: PathBuf,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        let project_path_task =
            Workspace::project_path_for_path(self.project.clone(), &abs_path, visible, cx);
        cx.spawn_in(window, async move |this, cx| {
            let (_, path) = project_path_task.await?;
            this.update_in(cx, |this, window, cx| this.split_path(path, window, cx))?
                .await
        })
    }

    // pub fn open_path
    /// Passing `None` for `pane` uses the default destination and honors
    /// `reveal_if_open`. Passing a pane explicitly limits reuse and opening to
    /// that pane.
    pub fn open_path(
        &mut self,
        path: impl Into<ProjectPath>,
        pane: Option<WeakEntity<Pane>>,
        focus_item: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        self.open_path_preview(path, pane, focus_item, false, true, window, cx)
    }
    // pub fn open_path_preview
    pub fn open_path_preview(
        &mut self,
        path: impl Into<ProjectPath>,
        pane: Option<WeakEntity<Pane>>,
        focus_item: bool,
        allow_preview: bool,
        activate: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        let reveal_if_open = pane.is_none() && WorkspaceSettings::get_global(cx).reveal_if_open;
        let requested_pane = pane.unwrap_or_else(|| {
            self.last_active_center_pane.clone().unwrap_or_else(|| {
                self.panes
                    .first()
                    .expect("There must be an active pane")
                    .downgrade()
            })
        });

        let workspace = self.weak_self.clone();
        let project_path = path.into();
        let task = self.load_path(project_path.clone(), window, cx);
        window.spawn(cx, async move |cx| {
            let (project_entry_id, build_item) = task.await?;
            let pane = if reveal_if_open {
                workspace
                    .read_with(cx, |workspace, cx| {
                        workspace.pane_containing_project_item(
                            &requested_pane,
                            project_entry_id,
                            &project_path,
                            cx,
                        )
                    })
                    .ok()
                    .flatten()
                    .map(|pane| pane.downgrade())
                    .unwrap_or(requested_pane)
            } else {
                requested_pane
            };

            pane.update_in(cx, |pane, window, cx| {
                pane.open_item(
                    project_entry_id,
                    project_path,
                    focus_item,
                    allow_preview,
                    activate,
                    None,
                    window,
                    cx,
                    build_item,
                )
            })
        })
    }
    // pub fn split_path
    pub fn split_path(
        &mut self,
        path: impl Into<ProjectPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        self.split_path_preview(path, false, None, window, cx)
    }
    // pub fn split_path_preview
    pub fn split_path_preview(
        &mut self,
        path: impl Into<ProjectPath>,
        allow_preview: bool,
        split_direction: Option<SplitDirection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<Box<dyn ItemHandle>>> {
        let pane = self.last_active_center_pane.clone().unwrap_or_else(|| {
            self.panes
                .first()
                .expect("There must be an active pane")
                .downgrade()
        });

        if let Member::Pane(center_pane) = &self.center.root
            && center_pane.read(cx).items_len() == 0
        {
            return self.open_path(path, Some(pane), true, window, cx);
        }

        let project_path = path.into();
        let task = self.load_path(project_path.clone(), window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let (project_entry_id, build_item) = task.await?;
            this.update_in(cx, move |this, window, cx| -> Option<_> {
                let pane = pane.upgrade()?;
                let new_pane = this.split_pane(
                    pane,
                    split_direction.unwrap_or(SplitDirection::Right),
                    window,
                    cx,
                );
                new_pane.update(cx, |new_pane, cx| {
                    Some(new_pane.open_item(
                        project_entry_id,
                        project_path,
                        true,
                        allow_preview,
                        true,
                        None,
                        window,
                        cx,
                        build_item,
                    ))
                })
            })
            .map(|option| option.context("pane was dropped"))?
        })
    }
    // pub fn load_path                 // private
    fn load_path(
        &mut self,
        path: ProjectPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<(Option<ProjectEntryId>, WorkspaceItemBuilder)>> {
        let registry = cx.default_global::<ProjectItemRegistry>().clone();
        registry.open_path(self.project(), &path, window, cx)
    }
    // pub fn find_project_item<T>
    pub fn find_project_item<T>(
        &self,
        pane: &Entity<Pane>,
        project_item: &Entity<T::Item>,
        cx: &App,
    ) -> Option<Entity<T>>
    where
        T: ProjectItem,
    {
        use project::ProjectItem as _;
        let project_item = project_item.read(cx);
        let entry_id = project_item.entry_id(cx);
        let project_path = project_item.project_path(cx);

        let mut item = None;
        if let Some(entry_id) = entry_id {
            item = pane.read(cx).item_for_entry(entry_id, cx);
        }
        if item.is_none()
            && let Some(project_path) = project_path
        {
            item = pane.read(cx).item_for_path(project_path, cx);
        }

        item.and_then(|item| item.downcast::<T>())
    }
    // pub fn is_project_item_open<T>
    pub fn is_project_item_open<T>(
        &self,
        pane: &Entity<Pane>,
        project_item: &Entity<T::Item>,
        cx: &App,
    ) -> bool
    where
        T: ProjectItem,
    {
        self.find_project_item::<T>(pane, project_item, cx)
            .is_some()
    }
    // pub fn open_project_item<T>
    /// Passing `None` for `pane` uses the active pane as the default destination
    /// and honors `reveal_if_open`. Passing a pane explicitly limits reuse and
    /// opening to that pane.
    pub fn open_project_item<T>(
        &mut self,
        pane: Option<Entity<Pane>>,
        project_item: Entity<T::Item>,
        activate_pane: bool,
        focus_item: bool,
        keep_old_preview: bool,
        allow_new_preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<T>
    where
        T: ProjectItem,
    {
        let reveal_if_open = pane.is_none() && WorkspaceSettings::get_global(cx).reveal_if_open;
        let requested_pane = pane.unwrap_or_else(|| self.active_pane.clone());
        let existing_item = self
            .find_project_item(&requested_pane, &project_item, cx)
            .map(|item| (requested_pane.clone(), item))
            .or_else(|| {
                if reveal_if_open {
                    self.panes.iter().find_map(|pane| {
                        if pane == &requested_pane {
                            None
                        } else {
                            self.find_project_item(pane, &project_item, cx)
                                .map(|item| (pane.clone(), item))
                        }
                    })
                } else {
                    None
                }
            });
        let pane = existing_item
            .as_ref()
            .map(|(pane, _)| pane.clone())
            .unwrap_or(requested_pane);
        let old_item_id = pane.read(cx).active_item().map(|item| item.item_id());

        if let Some((_, item)) = existing_item {
            if !keep_old_preview
                && let Some(old_id) = old_item_id
                && old_id != item.item_id()
            {
                // switching to a different item, so unpreview old active item
                pane.update(cx, |pane, _| {
                    pane.unpreview_item_if_preview(old_id);
                });
            }

            self.activate_item(&item, activate_pane, focus_item, window, cx);
            if !allow_new_preview {
                pane.update(cx, |pane, _| {
                    pane.unpreview_item_if_preview(item.item_id());
                });
            }
            return item;
        }

        let item = pane.update(cx, |pane, cx| {
            cx.new(|cx| {
                T::for_project_item(self.project().clone(), Some(pane), project_item, window, cx)
            })
        });
        let mut destination_index = None;
        pane.update(cx, |pane, cx| {
            if !keep_old_preview && let Some(old_id) = old_item_id {
                pane.unpreview_item_if_preview(old_id);
            }
            if allow_new_preview {
                destination_index = pane.replace_preview_item_id(item.item_id(), window, cx);
            }
        });

        self.add_item(
            pane,
            Box::new(item.clone()),
            destination_index,
            activate_pane,
            focus_item,
            window,
            cx,
        );
        item
    }
    // fn pane_containing_project_item  // private
    fn pane_containing_project_item(
        &self,
        requested_pane: &WeakEntity<Pane>,
        project_entry_id: Option<ProjectEntryId>,
        project_path: &ProjectPath,
        cx: &App,
    ) -> Option<Entity<Pane>> {
        let pane_contains_project_item = |pane: &Entity<Pane>| {
            pane.read(cx).items().any(|item| {
                if item.buffer_kind(cx) != ItemBufferKind::Singleton {
                    return false;
                }

                if let Some(project_entry_id) = project_entry_id {
                    item.project_entry_ids(cx).as_slice() == [project_entry_id]
                } else {
                    item.project_path(cx).as_ref() == Some(project_path)
                }
            })
        };

        let requested_pane = requested_pane.upgrade();
        if let Some(requested_pane) = requested_pane.as_ref()
            && pane_contains_project_item(requested_pane)
        {
            return Some(requested_pane.clone());
        }

        self.panes.iter().find_map(|pane| {
            if requested_pane.as_ref() == Some(pane) || !pane_contains_project_item(pane) {
                None
            } else {
                Some(pane.clone())
            }
        })
    }
    // pub fn open_url_or_file
    /// Opens a URL or file path, intelligently routing to the appropriate handler:
    ///
    /// - `http://` and `https://` URLs are opened in the system's default browser
    /// - `file://` URIs are opened as files in the editor
    /// - Paths without a URL scheme are treated as file paths:
    ///   - Absolute paths are opened directly
    ///   - Relative paths are first resolved against `base_path` (if provided),
    ///     then against visible project worktrees
    /// - Other URI schemes (e.g., `mailto:`, `vscode:`) are passed to the system handler
    ///
    /// # Arguments
    /// * `url_or_path` - The URL or file path to open
    /// * `base_path` - Optional base directory for resolving relative paths (e.g., the
    ///   directory containing a markdown file). If not provided, relative paths are
    ///   resolved against project worktrees.
    ///
    /// This method provides a unified way to handle links that may be either URLs
    /// or file paths, such as those found in markdown documents, terminal output,
    /// or LSP responses.
    pub fn open_url_or_file(
        &mut self,
        url_or_path: &str,
        base_path: Option<&Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut open_abs_path = |this: &mut Self, path, cx: &mut _| {
            let url_or_path = url_or_path.to_owned();
            let task = this.open_abs_path(
                path,
                OpenOptions {
                    visible: Some(OpenVisible::None),
                    ..Default::default()
                },
                window,
                cx,
            );
            (**cx)
                .spawn(async move |cx| {
                    if task.await.is_err() {
                        cx.update(|cx| cx.open_url(&url_or_path));
                    }
                })
                .detach();
        };

        if let Ok(url) = Url::parse(url_or_path) {
            match url.scheme() {
                "http" | "https" => cx.open_url(url_or_path),
                "file" => open_abs_path(self, PathBuf::from(url.path()), cx),
                _ => cx.open_url(url_or_path),
            }
            return;
        }

        // Not a valid URL - treat as a file path
        let project = self.project();
        let path_style = project.read(cx).path_style(cx);
        let project_is_local = project.read(cx).is_local();

        // If it's an absolute path, open it directly
        if path_style.is_absolute(url_or_path) {
            open_abs_path(self, PathBuf::from(url_or_path), cx);
            return;
        }

        let path = Path::new(url_or_path);
        // Try to resolve relative path against base_path first
        let path_from_base = if let Some(base) = base_path {
            let resolved_path = path_style.join(base, path).map(PathBuf::from);
            if project_is_local {
                if let Some(resolved_path) = resolved_path
                    && resolved_path.exists()
                {
                    open_abs_path(self, resolved_path, cx);
                    return;
                }
                None
            } else {
                resolved_path
            }
        } else {
            None
        };

        // Try to resolve against project worktrees
        let project_path = project.update(cx, |project, cx| {
            path_from_base
                .as_deref()
                .and_then(|base_path| project.find_project_path(base_path, cx))
                .or_else(|| project.find_project_path(path, cx))
        });
        if let Some(project_path) = project_path {
            let url_or_path = url_or_path.to_owned();
            let task = self.open_path(project_path, None, true, window, cx);
            (**cx)
                .spawn(async move |cx| {
                    if task.await.is_err() {
                        cx.update(|cx| cx.open_url(&url_or_path));
                    }
                })
                .detach();
            return;
        }

        // Couldn't resolve as a file path - try opening as URL anyway
        // (the OS might be able to handle it)
        cx.open_url(url_or_path);
    }
    // // 加入 pane
    // pub fn add_item_to_center
    pub fn add_item_to_center(
        &mut self,
        item: Box<dyn ItemHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(center_pane) = self.last_active_center_pane.clone() {
            if let Some(center_pane) = center_pane.upgrade() {
                center_pane.update(cx, |pane, cx| {
                    pane.add_item(item, true, true, None, window, cx)
                });
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    // pub fn add_item_to_active_pane
    pub fn add_item_to_active_pane(
        &mut self,
        item: Box<dyn ItemHandle>,
        destination_index: Option<usize>,
        focus_item: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.add_item(
            self.active_pane.clone(),
            item,
            destination_index,
            false,
            focus_item,
            window,
            cx,
        )
    }
    // pub fn add_item
    pub fn add_item(
        &mut self,
        pane: Entity<Pane>,
        item: Box<dyn ItemHandle>,
        destination_index: Option<usize>,
        activate_pane: bool,
        focus_item: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        pane.update(cx, |pane, cx| {
            pane.add_item(
                item,
                activate_pane,
                focus_item,
                destination_index,
                window,
                cx,
            )
        });
    }
    // pub fn split_item
    pub fn split_item(
        &mut self,
        split_direction: SplitDirection,
        item: Box<dyn ItemHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_pane = self.split_pane(self.active_pane.clone(), split_direction, window, cx);
        self.add_item(new_pane, item, None, true, true, window, cx);
    }

    // pub fn activate_item
    pub fn activate_item(
        &mut self,
        item: &dyn ItemHandle,
        activate_pane: bool,
        focus_item: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let result = self.panes.iter().find_map(|pane| {
            pane.read(cx)
                .index_for_item(item)
                .map(|ix| (pane.clone(), ix))
        });
        if let Some((pane, ix)) = result {
            pane.update(cx, |pane, cx| {
                pane.activate_item(ix, activate_pane, focus_item, window, cx)
            });
            true
        } else {
            false
        }
    }
    // pub fn move_item_to_pane_at_index
    fn move_item_to_pane_at_index(
        &mut self,
        action: &MoveItemToPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panes = self.center.panes();
        let destination = match panes.get(action.destination) {
            Some(&destination) => destination.clone(),
            None => {
                if !action.clone && self.active_pane.read(cx).items_len() < 2 {
                    return;
                }
                let direction = SplitDirection::Right;
                let split_off_pane = self
                    .find_pane_in_direction(direction, cx)
                    .unwrap_or_else(|| self.active_pane.clone());
                let new_pane = self.add_pane(window, cx);
                self.center.split(&split_off_pane, &new_pane, direction, cx);
                new_pane
            }
        };

        if action.clone {
            if self
                .active_pane
                .read(cx)
                .active_item()
                .is_some_and(|item| item.can_split(cx))
            {
                clone_active_item(
                    self.database_id(),
                    &self.active_pane,
                    &destination,
                    action.focus,
                    window,
                    cx,
                );
                return;
            }
        }
        move_active_item(
            &self.active_pane,
            &destination,
            action.focus,
            true,
            window,
            cx,
        )
    }
    // pub fn move_item_to_pane_in_direction
    pub fn move_item_to_pane_in_direction(
        &mut self,
        action: &MoveItemToPaneInDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let destination = match self.find_pane_in_direction(action.direction, cx) {
            Some(destination) => destination,
            None => {
                if !action.clone && self.active_pane.read(cx).items_len() < 2 {
                    return;
                }
                let new_pane = self.add_pane(window, cx);
                self.center
                    .split(&self.active_pane, &new_pane, action.direction, cx);
                new_pane
            }
        };

        if action.clone {
            if self
                .active_pane
                .read(cx)
                .active_item()
                .is_some_and(|item| item.can_split(cx))
            {
                clone_active_item(
                    self.database_id(),
                    &self.active_pane,
                    &destination,
                    action.focus,
                    window,
                    cx,
                );
                return;
            }
        }
        move_active_item(
            &self.active_pane,
            &destination,
            action.focus,
            true,
            window,
            cx,
        );
    }


}
