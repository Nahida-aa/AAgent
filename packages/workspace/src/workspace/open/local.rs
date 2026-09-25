impl Workspace {
    // pub fn with_local_workspace
    /// Call the given callback with a workspace whose project is local or remote via WSL (allowing host access).
    ///
    /// If the given workspace has a local project, then it will be passed
    /// to the callback. Otherwise, a new empty window will be created.
    pub fn with_local_workspace<T, F>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        callback: F,
    ) -> Task<Result<T>>
    where
        T: 'static,
        F: 'static + FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) -> T,
    {
        if self.project.read(cx).is_local() {
            Task::ready(Ok(callback(self, window, cx)))
        } else {
            let env = self.project.read(cx).cli_environment(cx);
            let task = Self::new_local(
                Vec::new(),
                self.app_state.clone(),
                None,
                env,
                None,
                OpenMode::Activate,
                cx,
            );
            cx.spawn_in(window, async move |_vh, cx| {
                let OpenResult {
                    window: multi_workspace_window,
                    ..
                } = task.await?;
                multi_workspace_window.update(cx, |multi_workspace, window, cx| {
                    let workspace = multi_workspace.workspace().clone();
                    workspace.update(cx, |workspace, cx| callback(workspace, window, cx))
                })
            })
        }
    }
    // pub fn with_local_or_wsl_workspace
    /// Call the given callback with a workspace whose project is local or remote via WSL (allowing host access).
    ///
    /// If the given workspace has a local project, then it will be passed
    /// to the callback. Otherwise, a new empty window will be created.
    pub fn with_local_or_wsl_workspace<T, F>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        callback: F,
    ) -> Task<Result<T>>
    where
        T: 'static,
        F: 'static + FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) -> T,
    {
        let project = self.project.read(cx);
        if project.is_local() || project.is_via_wsl_with_host_interop(cx) {
            Task::ready(Ok(callback(self, window, cx)))
        } else {
            let env = self.project.read(cx).cli_environment(cx);
            let task = Self::new_local(
                Vec::new(),
                self.app_state.clone(),
                None,
                env,
                None,
                OpenMode::Activate,
                cx,
            );
            cx.spawn_in(window, async move |_vh, cx| {
                let OpenResult {
                    window: multi_workspace_window,
                    ..
                } = task.await?;
                multi_workspace_window.update(cx, |multi_workspace, window, cx| {
                    let workspace = multi_workspace.workspace().clone();
                    workspace.update(cx, |workspace, cx| callback(workspace, window, cx))
                })
            })
        }
    }
    // pub fn open_workspace_for_paths
    pub fn open_workspace_for_paths(
        &mut self,
        // replace_current_window: bool,
        mut open_mode: OpenMode,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Workspace>>> {
        let requesting_window = window.window_handle().downcast::<MultiWorkspace>();
        let is_remote = self.project.read(cx).is_via_collab();
        let has_worktree = self.project.read(cx).worktrees(cx).next().is_some();
        let has_dirty_items = self.items(cx).any(|item| item.is_dirty(cx));

        let workspace_is_empty = !is_remote && !has_worktree && !has_dirty_items;
        if workspace_is_empty {
            open_mode = OpenMode::Activate;
        }

        let app_state = self.app_state.clone();

        cx.spawn(async move |_, cx| {
            let OpenResult { workspace, .. } = cx
                .update(|cx| {
                    open_paths(
                        &paths,
                        app_state,
                        OpenOptions {
                            requesting_window,
                            open_mode,
                            workspace_matching: if open_mode == OpenMode::NewWindow {
                                WorkspaceMatching::None
                            } else {
                                WorkspaceMatching::default()
                            },
                            ..Default::default()
                        },
                        cx,
                    )
                })
                .await?;
            Ok(workspace)
        })
    }
    // pub fn open_paths
    #[allow(clippy::type_complexity)]
    pub fn open_paths(
        &mut self,
        mut abs_paths: Vec<PathBuf>,
        options: OpenOptions,
        pane: Option<WeakEntity<Pane>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Vec<Option<anyhow::Result<Box<dyn ItemHandle>>>>> {
        let fs = self.app_state.fs.clone();

        let caller_ordered_abs_paths = abs_paths.clone();

        // Sort the paths to ensure we add worktrees for parents before their children.
        abs_paths.sort_unstable();
        cx.spawn_in(window, async move |this, cx| {
            let mut tasks = Vec::with_capacity(abs_paths.len());

            for abs_path in &abs_paths {
                let visible = match options.visible.as_ref().unwrap_or(&OpenVisible::None) {
                    OpenVisible::All => Some(true),
                    OpenVisible::None => Some(false),
                    OpenVisible::OnlyFiles => match fs.metadata(abs_path).await.log_err() {
                        Some(Some(metadata)) => Some(!metadata.is_dir),
                        Some(None) => Some(true),
                        None => None,
                    },
                    OpenVisible::OnlyDirectories => match fs.metadata(abs_path).await.log_err() {
                        Some(Some(metadata)) => Some(metadata.is_dir),
                        Some(None) => Some(false),
                        None => None,
                    },
                };
                let project_path = match visible {
                    Some(visible) => match this
                        .update(cx, |this, cx| {
                            Workspace::project_path_for_path(
                                this.project.clone(),
                                abs_path,
                                visible,
                                cx,
                            )
                        })
                        .log_err()
                    {
                        Some(project_path) => project_path.await.log_err(),
                        None => None,
                    },
                    None => None,
                };

                let this = this.clone();
                let abs_path: Arc<Path> = SanitizedPath::new(&abs_path).as_path().into();
                let fs = fs.clone();
                let pane = pane.clone();
                let task = cx.spawn(async move |cx| {
                    let (worktree, project_path) = project_path?;
                    let (entry_is_directory, worktree_is_local) =
                        worktree.read_with(cx, |worktree, _| {
                            let entry = if project_path.path.as_unix_str().is_empty() {
                                worktree.root_entry()
                            } else {
                                worktree.entry_for_path(&project_path.path)
                            };
                            (entry.map(|entry| entry.is_dir()), worktree.is_local())
                        });
                    let is_directory = match entry_is_directory {
                        Some(is_directory) => is_directory,
                        None if worktree_is_local => fs.is_dir(&abs_path).await,
                        None => false,
                    };

                    if is_directory {
                        // Opening a directory should not race to update the active entry.
                        // We'll select/reveal a deterministic final entry after all paths finish opening.
                        None
                    } else {
                        Some(
                            this.update_in(cx, |this, window, cx| {
                                this.open_path(
                                    project_path,
                                    pane,
                                    options.focus.unwrap_or(true),
                                    window,
                                    cx,
                                )
                            })
                            .ok()?
                            .await,
                        )
                    }
                });
                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;

            // Determine the winner using the fake/abstract FS metadata, not `Path::is_dir`.
            let mut winner: Option<(PathBuf, bool)> = None;
            for abs_path in caller_ordered_abs_paths.into_iter().rev() {
                if let Some(Some(metadata)) = fs.metadata(&abs_path).await.log_err() {
                    if !metadata.is_dir {
                        winner = Some((abs_path, false));
                        break;
                    }
                    if winner.is_none() {
                        winner = Some((abs_path, true));
                    }
                } else if winner.is_none() {
                    winner = Some((abs_path, false));
                }
            }

            // Compute the winner entry id on the foreground thread and emit once, after all
            // paths finish opening. This avoids races between concurrently-opening paths
            // (directories in particular) and makes the resulting project panel selection
            // deterministic.
            if let Some((winner_abs_path, winner_is_dir)) = winner {
                'emit_winner: {
                    let winner_abs_path: Arc<Path> =
                        SanitizedPath::new(&winner_abs_path).as_path().into();

                    let visible = match options.visible.as_ref().unwrap_or(&OpenVisible::None) {
                        OpenVisible::All => true,
                        OpenVisible::None => false,
                        OpenVisible::OnlyFiles => !winner_is_dir,
                        OpenVisible::OnlyDirectories => winner_is_dir,
                    };

                    let Some(worktree_task) = this
                        .update(cx, |workspace, cx| {
                            workspace.project.update(cx, |project, cx| {
                                project.find_or_create_worktree(
                                    winner_abs_path.as_ref(),
                                    visible,
                                    cx,
                                )
                            })
                        })
                        .ok()
                    else {
                        break 'emit_winner;
                    };

                    let Ok((worktree, _)) = worktree_task.await else {
                        break 'emit_winner;
                    };

                    let Ok(Some(entry_id)) = this.update(cx, |_, cx| {
                        let worktree = worktree.read(cx);
                        let worktree_abs_path = worktree.abs_path();
                        let entry = if winner_abs_path.as_ref() == worktree_abs_path.as_ref() {
                            worktree.root_entry()
                        } else {
                            winner_abs_path
                                .strip_prefix(worktree_abs_path.as_ref())
                                .ok()
                                .and_then(|relative_path| {
                                    let relative_path =
                                        RelPath::new(relative_path, PathStyle::local())
                                            .log_err()?;
                                    worktree.entry_for_path(&relative_path)
                                })
                        }?;
                        Some(entry.id)
                    }) else {
                        break 'emit_winner;
                    };

                    this.update(cx, |workspace, cx| {
                        workspace.project.update(cx, |_, cx| {
                            cx.emit(project::Event::ActiveEntryChanged(Some(entry_id)));
                        });
                    })
                    .ok();
                }
            }

            results
        })
    }
}

fn open_items(
    serialized_workspace: Option<SerializedWorkspace>,
    mut project_paths_to_open: Vec<(PathBuf, Option<ProjectPath>)>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> impl 'static + Future<Output = Result<Vec<Option<Result<Box<dyn ItemHandle>>>>>> + use<> {
    let restored_items = serialized_workspace.map(|serialized_workspace| {
        Workspace::load_workspace(
            serialized_workspace,
            project_paths_to_open
                .iter()
                .map(|(_, project_path)| project_path)
                .cloned()
                .collect(),
            window,
            cx,
        )
    });

    cx.spawn_in(window, async move |workspace, cx| {
        let mut opened_items = Vec::with_capacity(project_paths_to_open.len());

        if let Some(restored_items) = restored_items {
            let restored_items = restored_items.await?;

            let restored_project_paths = restored_items
                .iter()
                .filter_map(|item| {
                    cx.update(|_, cx| item.as_ref()?.project_path(cx))
                        .ok()
                        .flatten()
                })
                .collect::<HashSet<_>>();

            for restored_item in restored_items {
                opened_items.push(restored_item.map(Ok));
            }

            project_paths_to_open
                .iter_mut()
                .for_each(|(_, project_path)| {
                    if let Some(project_path_to_open) = project_path
                        && restored_project_paths.contains(project_path_to_open)
                    {
                        *project_path = None;
                    }
                });
        } else {
            for _ in 0..project_paths_to_open.len() {
                opened_items.push(None);
            }
        }
        assert!(opened_items.len() == project_paths_to_open.len());

        let tasks =
            project_paths_to_open
                .into_iter()
                .enumerate()
                .map(|(ix, (abs_path, project_path))| {
                    let workspace = workspace.clone();
                    cx.spawn(async move |cx| {
                        let file_project_path = project_path?;
                        let abs_path_task = workspace.update(cx, |workspace, cx| {
                            workspace.project().update(cx, |project, cx| {
                                project.resolve_abs_path(abs_path.to_string_lossy().as_ref(), cx)
                            })
                        });

                        // We only want to open file paths here. If one of the items
                        // here is a directory, it was already opened further above
                        // with a `find_or_create_worktree`.
                        if let Ok(task) = abs_path_task
                            && task.await.is_none_or(|p| p.is_file())
                        {
                            return Some((
                                ix,
                                workspace
                                    .update_in(cx, |workspace, window, cx| {
                                        workspace.open_path(
                                            file_project_path,
                                            None,
                                            true,
                                            window,
                                            cx,
                                        )
                                    })
                                    .log_err()?
                                    .await,
                            ));
                        }
                        None
                    })
                });

        let tasks = tasks.collect::<Vec<_>>();

        let tasks = futures::future::join_all(tasks);
        for (ix, path_open_result) in tasks.await.into_iter().flatten() {
            opened_items[ix] = Some(path_open_result);
        }

        Ok(opened_items)
    })
}

/// Opens a workspace by its database ID, used for restoring empty workspaces with unsaved content.
pub fn open_workspace_by_id(
    workspace_id: WorkspaceId,
    app_state: Arc<AppState>,
    requesting_window: Option<WindowHandle<MultiWorkspace>>,
    cx: &mut App,
) -> Task<anyhow::Result<WindowHandle<MultiWorkspace>>> {
    let project_handle = Project::local(
        app_state.client.clone(),
        app_state.node_runtime.clone(),
        app_state.user_store.clone(),
        app_state.languages.clone(),
        app_state.fs.clone(),
        None,
        project::LocalProjectFlags {
            init_worktree_trust: true,
            ..project::LocalProjectFlags::default()
        },
        cx,
    );

    let db = WorkspaceDb::global(cx);
    let kvp = db::kvp::KeyValueStore::global(cx);
    cx.spawn(async move |cx| {
        let serialized_workspace = db
            .workspace_for_id(workspace_id)
            .with_context(|| format!("Workspace {workspace_id:?} not found"))?;

        let centered_layout = serialized_workspace.centered_layout;

        let (window, workspace) = if let Some(window) = requesting_window {
            let workspace = window.update(cx, |multi_workspace, window, cx| {
                let workspace = cx.new(|cx| {
                    let mut workspace = Workspace::new(
                        Some(workspace_id),
                        project_handle.clone(),
                        app_state.clone(),
                        window,
                        cx,
                    );
                    workspace.centered_layout = centered_layout;
                    workspace
                });
                multi_workspace.add(workspace.clone(), &*window, cx);
                workspace
            })?;
            (window, workspace)
        } else {
            let window_bounds_override = window_bounds_env_override();

            let (window_bounds, display) = if let Some(bounds) = window_bounds_override {
                (Some(WindowBounds::Windowed(bounds)), None)
            } else if let Some(display) = serialized_workspace.display
                && let Some(bounds) = serialized_workspace.window_bounds.as_ref()
            {
                (Some(bounds.0), Some(display))
            } else if let Some((display, bounds)) = persistence::read_default_window_bounds(&kvp) {
                (Some(bounds), Some(display))
            } else {
                (None, None)
            };

            let options = cx.update(|cx| {
                let mut options = (app_state.build_window_options)(display, cx);
                options.window_bounds = window_bounds;
                options
            });

            let window = cx.open_window(options, {
                let app_state = app_state.clone();
                let project_handle = project_handle.clone();
                move |window, cx| {
                    let workspace = cx.new(|cx| {
                        let mut workspace = Workspace::new(
                            Some(workspace_id),
                            project_handle,
                            app_state,
                            window,
                            cx,
                        );
                        workspace.centered_layout = centered_layout;
                        workspace
                    });
                    cx.new(|cx| MultiWorkspace::new(workspace, window, cx))
                }
            })?;

            let workspace = window.update(cx, |multi_workspace: &mut MultiWorkspace, _, _cx| {
                multi_workspace.workspace().clone()
            })?;

            (window, workspace)
        };

        notify_if_database_failed(window, cx);

        // Restore items from the serialized workspace
        window
            .update(cx, |_, window, cx| {
                workspace.update(cx, |_workspace, cx| {
                    open_items(Some(serialized_workspace), vec![], window, cx)
                })
            })?
            .await?;

        window.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.serialize_workspace(window, cx);
            });
        })?;

        Ok(window)
    })
}

#[allow(clippy::type_complexity)]
pub fn open_paths(
    abs_paths: &[PathBuf],
    app_state: Arc<AppState>,
    mut open_options: OpenOptions,
    cx: &mut App,
) -> Task<anyhow::Result<OpenResult>> {
    let abs_paths = abs_paths.to_vec();
    #[cfg(target_os = "windows")]
    let wsl_path = abs_paths
        .iter()
        .find_map(|p| util::paths::WslPath::from_path(p));

    cx.spawn(async move |cx| {
        let (mut existing, mut open_visible) = find_existing_workspace(
            &abs_paths,
            &open_options,
            &SerializedWorkspaceLocation::Local,
            cx,
        )
        .await;

        // Fallback: if no workspace contains the paths and all paths are files,
        // prefer an existing local workspace window (active window first).
        if open_options.should_reuse_existing_window() && existing.is_none() {
            let all_paths = abs_paths.iter().map(|path| app_state.fs.metadata(path));
            let all_metadatas = futures::future::join_all(all_paths)
                .await
                .into_iter()
                .filter_map(|result| result.ok().flatten());

            if all_metadatas.into_iter().all(|file| !file.is_dir) {
                cx.update(|cx| {
                    let windows = workspace_windows_for_location(
                        &SerializedWorkspaceLocation::Local,
                        cx,
                    );
                    let window = cx
                        .active_window()
                        .and_then(|window| window.downcast::<MultiWorkspace>())
                        .filter(|window| windows.contains(window))
                        .or_else(|| windows.into_iter().next());
                    if let Some(window) = window {
                        if let Ok(multi_workspace) = window.read(cx) {
                            let active_workspace = multi_workspace.workspace().clone();
                            existing = Some((window, active_workspace));
                            open_visible = OpenVisible::None;
                        }
                    }
                });
            }
        }

        // Fallback for directories: when no flag is specified and no existing
        // workspace matched, check the user's setting to decide whether to add
        // the directory as a new workspace in the active window's MultiWorkspace
        // or open a new window.
        // Skip when requesting_window is already set: the caller (e.g.
        // open_workspace_for_paths reusing an empty window) already chose the
        // target window, so we must not open the sidebar as a side-effect.
        if open_options.should_reuse_existing_window()
            && existing.is_none()
            && open_options.requesting_window.is_none()
        {
            let use_existing_window = open_options.add_dirs_to_sidebar;

            if use_existing_window {
                let target_window = cx.update(|cx| {
                    let windows = workspace_windows_for_location(
                        &SerializedWorkspaceLocation::Local,
                        cx,
                    );
                    let window = cx
                        .active_window()
                        .and_then(|window| window.downcast::<MultiWorkspace>())
                        .filter(|window| windows.contains(window))
                        .or_else(|| windows.into_iter().next());
                    window.filter(|window| {
                        window
                            .read(cx)
                            .is_ok_and(|mw| mw.multi_workspace_enabled(cx))
                    })
                });

                if let Some(window) = target_window {
                    open_options.requesting_window = Some(window);
                    window
                        .update(cx, |multi_workspace, _, cx| {
                            multi_workspace.open_sidebar(cx);
                        })
                        .log_err();
                }
            }
        }

        let open_in_dev_container = open_options.open_in_dev_container;

        let result = if let Some((existing, target_workspace)) = existing {
            let open_task = existing
                .update(cx, |multi_workspace, window, cx| {
                    window.activate_window();
                    multi_workspace.activate(target_workspace.clone(), None, window, cx);
                    target_workspace.update(cx, |workspace, cx| {
                        if open_in_dev_container {
                            workspace.set_open_in_dev_container(true);
                        }
                        workspace.open_paths(
                            abs_paths,
                            OpenOptions {
                                visible: Some(open_visible),
                                ..Default::default()
                            },
                            None,
                            window,
                            cx,
                        )
                    })
                })?
                .await;

            _ = existing.update(cx, |multi_workspace, _, cx| {
                let workspace = multi_workspace.workspace().clone();
                workspace.update(cx, |workspace, cx| {
                    for item in open_task.iter().flatten() {
                        if let Err(e) = item {
                            workspace.show_error(format!("Error: {e}"), cx);
                        }
                    }
                });
            });

            Ok(OpenResult { window: existing, workspace: target_workspace, opened_items: open_task })
        } else {
            let init = if open_in_dev_container {
                Some(Box::new(|workspace: &mut Workspace, _window: &mut Window, _cx: &mut Context<Workspace>| {
                    workspace.set_open_in_dev_container(true);
                }) as Box<dyn FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + Send>)
            } else {
                None
            };
            let result = cx
                .update(move |cx| {
                    Workspace::new_local(
                        abs_paths,
                        app_state.clone(),
                        open_options.requesting_window,
                        open_options.env,
                        init,
                        open_options.open_mode,
                        cx,
                    )
                })
                .await;

            if let Ok(ref result) = result {
                result.window
                    .update(cx, |_, window, _cx| {
                        window.activate_window();
                    })
                    .log_err();
            }

            result
        };

        #[cfg(target_os = "windows")]
        if let Some(util::paths::WslPath{distro, path}) = wsl_path
            && let Ok(ref result) = result
        {
            result.window
                .update(cx, move |multi_workspace, _window, cx| {
                    struct OpenInWsl;
                    let workspace = multi_workspace.workspace().clone();
                    workspace.update(cx, |workspace, cx| {
                        workspace.show_notification(NotificationId::unique::<OpenInWsl>(), cx, move |cx| {
                            let display_path = util::markdown::MarkdownInlineCode(&path.to_string_lossy());
                            let msg = format!("{display_path} is inside a WSL filesystem, some features may not work unless you open it with WSL remote");
                            cx.new(move |cx| {
                                MessageNotification::new(msg, cx)
                                    .primary_message("Open in WSL")
                                    .primary_icon(IconName::FolderOpen)
                                    .primary_on_click(move |window, cx| {
                                        window.dispatch_action(Box::new(remote::OpenWslPath {
                                                distro: remote::WslConnectionOptions {
                                                        distro_name: distro.clone(),
                                                    user: None,
                                                },
                                                paths: vec![path.clone().into()],
                                            }), cx)
                                    })
                            })
                        });
                    });
                })
                .unwrap();
        };
        result
    })
}

pub fn open_new(
    open_options: OpenOptions,
    app_state: Arc<AppState>,
    cx: &mut App,
    init: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static + Send,
) -> Task<anyhow::Result<()>> {
    let addition = open_options.open_mode;
    let task = Workspace::new_local(
        Vec::new(),
        app_state,
        open_options.requesting_window,
        open_options.env,
        Some(Box::new(init)),
        addition,
        cx,
    );
    cx.spawn(async move |cx| {
        let OpenResult { window, .. } = task.await?;
        window
            .update(cx, |_, window, _cx| {
                window.activate_window();
            })
            .ok();
        Ok(())
    })
}
