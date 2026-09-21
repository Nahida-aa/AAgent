use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {}

pub fn join_channel(
    channel_id: ChannelId,
    app_state: Arc<AppState>,
    requesting_window: Option<WindowHandle<MultiWorkspace>>,
    requesting_workspace: Option<WeakEntity<Workspace>>,
    cx: &mut App,
) -> Task<Result<()>> {
    let active_call = GlobalAnyActiveCall::global(cx).clone();
    cx.spawn(async move |cx| {
        let result = join_channel_internal(
            channel_id,
            &app_state,
            requesting_window,
            requesting_workspace,
            &*active_call.0,
            cx,
        )
        .await;

        // join channel succeeded, and opened a window
        if matches!(result, Ok(true)) {
            return anyhow::Ok(());
        }

        // find an existing workspace to focus and show call controls
        let mut active_window = requesting_window.or_else(|| activate_any_workspace_window(cx));
        if active_window.is_none() {
            // no open workspaces, make one to show the error in (blergh)
            let OpenResult {
                window: window_handle,
                ..
            } = cx
                .update(|cx| {
                    Workspace::new_local(
                        vec![],
                        app_state.clone(),
                        requesting_window,
                        None,
                        None,
                        OpenMode::Activate,
                        cx,
                    )
                })
                .await?;

            window_handle
                .update(cx, |_, window, _cx| {
                    window.activate_window();
                })
                .ok();

            if result.is_ok() {
                cx.update(|cx| {
                    cx.dispatch_action(&OpenChannelNotes);
                });
            }

            active_window = Some(window_handle);
        }

        if let Err(err) = result {
            log::error!("failed to join channel: {}", err);
            if let Some(active_window) = active_window {
                active_window
                    .update(cx, |_, window, cx| {
                        let detail: SharedString = match err.error_code() {
                            ErrorCode::SignedOut => "Please sign in to continue.".into(),
                            ErrorCode::UpgradeRequired => concat!(
                                "Your are running an unsupported version of Zed. ",
                                "Please update to continue."
                            )
                            .into(),
                            ErrorCode::NoSuchChannel => concat!(
                                "No matching channel was found. ",
                                "Please check the link and try again."
                            )
                            .into(),
                            ErrorCode::Forbidden => concat!(
                                "This channel is private, and you do not have access. ",
                                "Please ask someone to add you and try again."
                            )
                            .into(),
                            ErrorCode::Disconnected => {
                                "Please check your internet connection and try again.".into()
                            }
                            _ => format!("{}\n\nPlease try again.", err).into(),
                        };
                        window.prompt(
                            PromptLevel::Critical,
                            "Failed to join channel",
                            Some(&detail),
                            &["OK"],
                            cx,
                        )
                    })?
                    .await
                    .ok();
            }
        }

        // return ok, we showed the error to the user.
        anyhow::Ok(())
    })
}

async fn join_channel_internal(
    channel_id: ChannelId,
    app_state: &Arc<AppState>,
    requesting_window: Option<WindowHandle<MultiWorkspace>>,
    requesting_workspace: Option<WeakEntity<Workspace>>,
    active_call: &dyn AnyActiveCall,
    cx: &mut AsyncApp,
) -> Result<bool> {
    let (should_prompt, already_in_channel) = cx.update(|cx| {
        if !active_call.is_in_room(cx) {
            return (false, false);
        }

        let already_in_channel = active_call.channel_id(cx) == Some(channel_id);
        let should_prompt = active_call.is_sharing_project(cx)
            && active_call.has_remote_participants(cx)
            && !already_in_channel;
        (should_prompt, already_in_channel)
    });

    if already_in_channel {
        let task = cx.update(|cx| {
            if let Some((project, host)) = active_call.most_active_project(cx) {
                Some(join_in_room_project(project, host, app_state.clone(), cx))
            } else {
                None
            }
        });
        if let Some(task) = task {
            task.await?;
        }
        return anyhow::Ok(true);
    }

    if should_prompt {
        if let Some(multi_workspace) = requesting_window {
            let answer = multi_workspace
                .update(cx, |_, window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        "Do you want to switch channels?",
                        Some("Leaving this call will unshare your current project."),
                        &["Yes, Join Channel", "Cancel"],
                        cx,
                    )
                })?
                .await;

            if answer == Ok(1) {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }

    let client = cx.update(|cx| active_call.client(cx));

    let mut client_status = client.status();

    // this loop will terminate within client::CONNECTION_TIMEOUT seconds.
    'outer: loop {
        let Some(status) = client_status.recv().await else {
            anyhow::bail!("error connecting");
        };

        match status {
            Status::Connecting
            | Status::Authenticating
            | Status::Authenticated
            | Status::Reconnecting
            | Status::Reauthenticating
            | Status::Reauthenticated => continue,
            Status::Connected { .. } => break 'outer,
            Status::SignedOut | Status::AuthenticationError => {
                return Err(ErrorCode::SignedOut.into());
            }
            Status::UpgradeRequired => return Err(ErrorCode::UpgradeRequired.into()),
            Status::ConnectionError | Status::ConnectionLost | Status::ReconnectionError { .. } => {
                return Err(ErrorCode::Disconnected.into());
            }
        }
    }

    let joined = cx
        .update(|cx| active_call.join_channel(channel_id, cx))
        .await?;

    if !joined {
        return anyhow::Ok(true);
    }

    cx.update(|cx| active_call.room_update_completed(cx)).await;

    let task = cx.update(|cx| {
        if let Some((project, host)) = active_call.most_active_project(cx) {
            return Some(join_in_room_project(project, host, app_state.clone(), cx));
        }

        // If you are the first to join a channel, see if you should share your project.
        if !active_call.has_remote_participants(cx)
            && !active_call.local_participant_is_guest(cx)
            && let Some(workspace) = requesting_workspace.as_ref().and_then(|w| w.upgrade())
        {
            let project = workspace.update(cx, |workspace, cx| {
                let project = workspace.project.read(cx);

                if !active_call.share_on_join(cx) {
                    return None;
                }

                if (project.is_local() || project.is_via_remote_server())
                    && project.visible_worktrees(cx).any(|tree| {
                        tree.read(cx)
                            .root_entry()
                            .is_some_and(|entry| entry.is_dir())
                    })
                {
                    Some(workspace.project.clone())
                } else {
                    None
                }
            });
            if let Some(project) = project {
                let share_task = active_call.share_project(project, cx);
                return Some(cx.spawn(async move |_cx| -> Result<()> {
                    share_task.await?;
                    Ok(())
                }));
            }
        }

        None
    });
    if let Some(task) = task {
        task.await?;
        return anyhow::Ok(true);
    }
    anyhow::Ok(false)
}

pub fn join_in_room_project(
    project_id: u64,
    follow_user_id: u64,
    app_state: Arc<AppState>,
    cx: &mut App,
) -> Task<Result<()>> {
    let windows = cx.windows();
    cx.spawn(async move |cx| {
        let existing_window_and_workspace: Option<(
            WindowHandle<MultiWorkspace>,
            Entity<Workspace>,
        )> = windows.into_iter().find_map(|window_handle| {
            window_handle
                .downcast::<MultiWorkspace>()
                .and_then(|window_handle| {
                    window_handle
                        .update(cx, |multi_workspace, _window, cx| {
                            multi_workspace
                                .workspaces()
                                .find(|workspace| {
                                    workspace.read(cx).project().read(cx).remote_id()
                                        == Some(project_id)
                                })
                                .map(|workspace| (window_handle, workspace.clone()))
                        })
                        .unwrap_or(None)
                })
        });

        let multi_workspace_window = if let Some((existing_window, target_workspace)) =
            existing_window_and_workspace
        {
            existing_window
                .update(cx, |multi_workspace, window, cx| {
                    multi_workspace.activate(target_workspace, None, window, cx);
                })
                .ok();
            existing_window
        } else {
            let active_call = cx.update(|cx| GlobalAnyActiveCall::global(cx).clone());
            let project = cx
                .update(|cx| {
                    active_call.0.join_project(
                        project_id,
                        app_state.languages.clone(),
                        app_state.fs.clone(),
                        cx,
                    )
                })
                .await?;

            let window_bounds_override = window_bounds_env_override();
            cx.update(|cx| {
                let mut options = (app_state.build_window_options)(None, cx);
                options.window_bounds = window_bounds_override.map(WindowBounds::Windowed);
                cx.open_window(options, |window, cx| {
                    let workspace = cx.new(|cx| {
                        Workspace::new(Default::default(), project, app_state.clone(), window, cx)
                    });
                    cx.new(|cx| MultiWorkspace::new(workspace, window, cx))
                })
            })?
        };

        multi_workspace_window.update(cx, |multi_workspace, window, cx| {
            cx.activate(true);
            window.activate_window();

            // We set the active workspace above, so this is the correct workspace.
            let workspace = multi_workspace.workspace().clone();
            workspace.update(cx, |workspace, cx| {
                let follow_peer_id = GlobalAnyActiveCall::try_global(cx)
                    .and_then(|call| call.0.peer_id_for_user_in_room(follow_user_id, cx))
                    .or_else(|| {
                        // If we couldn't follow the given user, follow the host instead.
                        let collaborator = workspace
                            .project()
                            .read(cx)
                            .collaborators()
                            .values()
                            .find(|collaborator| collaborator.is_host)?;
                        Some(collaborator.peer_id)
                    });

                if let Some(follow_peer_id) = follow_peer_id {
                    workspace.follow(follow_peer_id, window, cx);
                }
            });
        })?;

        anyhow::Ok(())
    })
}

pub fn open_remote_project_with_existing_connection(
    connection_options: RemoteConnectionOptions,
    project: Entity<Project>,
    paths: Vec<PathBuf>,
    app_state: Arc<AppState>,
    window: WindowHandle<MultiWorkspace>,
    provisional_project_group_key: Option<ProjectGroupKey>,
    source_workspace: Option<WeakEntity<Workspace>>,
    cx: &mut AsyncApp,
) -> Task<Result<(Entity<Workspace>, Vec<Option<Box<dyn ItemHandle>>>)>> {
    cx.spawn(async move |cx| {
        let (workspace_id, serialized_workspace) =
            deserialize_remote_project(connection_options.clone(), paths.clone(), cx).await?;

        open_remote_project_inner(
            project,
            paths,
            workspace_id,
            serialized_workspace,
            app_state,
            window,
            provisional_project_group_key,
            source_workspace,
            cx,
        )
        .await
    })
}

async fn open_remote_project_inner(
    project: Entity<Project>,
    paths: Vec<PathBuf>,
    workspace_id: WorkspaceId,
    serialized_workspace: Option<SerializedWorkspace>,
    app_state: Arc<AppState>,
    window: WindowHandle<MultiWorkspace>,
    provisional_project_group_key: Option<ProjectGroupKey>,
    source_workspace: Option<WeakEntity<Workspace>>,
    cx: &mut AsyncApp,
) -> Result<(Entity<Workspace>, Vec<Option<Box<dyn ItemHandle>>>)> {
    let mut project_paths_to_open = vec![];
    let mut project_path_errors = vec![];

    for path in paths {
        let result = cx
            .update(|cx| {
                Workspace::project_path_for_path(project.clone(), path.as_path(), true, cx)
            })
            .await;
        match result {
            Ok((_, project_path)) => {
                project_paths_to_open.push((path, Some(project_path)));
            }
            Err(error) => {
                project_path_errors.push(error);
            }
        };
    }

    if project_paths_to_open.is_empty() {
        return Err(project_path_errors.pop().context("no paths given")?);
    }

    let workspace = window.update(cx, |multi_workspace, window, cx| {
        let new_workspace = cx.new(|cx| {
            let mut workspace = Workspace::new(
                Some(workspace_id),
                project.clone(),
                app_state.clone(),
                window,
                cx,
            );
            workspace.update_history(cx);

            if let Some(ref serialized) = serialized_workspace {
                workspace.centered_layout = serialized.centered_layout;
            }

            workspace
        });

        if let Some(project_group_key) = provisional_project_group_key.clone() {
            multi_workspace.activate_provisional_workspace(
                new_workspace.clone(),
                project_group_key,
                window,
                cx,
            );
        } else {
            multi_workspace.activate(new_workspace.clone(), source_workspace, window, cx);
        }
        new_workspace
    })?;

    let db = cx.update(|cx| WorkspaceDb::global(cx));
    let toolchains = db.toolchains(workspace_id).await?;
    for (toolchain, worktree_path, path) in toolchains {
        project
            .update(cx, |this, cx| {
                let Some(worktree_id) =
                    this.find_worktree(&worktree_path, cx)
                        .and_then(|(worktree, rel_path)| {
                            if rel_path.is_empty() {
                                Some(worktree.read(cx).id())
                            } else {
                                None
                            }
                        })
                else {
                    return Task::ready(None);
                };

                this.activate_toolchain(ProjectPath { worktree_id, path }, toolchain, cx)
            })
            .await;
    }

    let items = window
        .update(cx, |_, window, cx| {
            window.activate_window();
            workspace.update(cx, |_workspace, cx| {
                open_items(serialized_workspace, project_paths_to_open, window, cx)
            })
        })?
        .await?;

    workspace.update(cx, |workspace, cx| {
        for error in project_path_errors {
            if error.error_code() == proto::ErrorCode::DevServerProjectPathDoesNotExist {
                if let Some(path) = error.error_tag("path") {
                    workspace.show_error(format!("'{path}' does not exist"), cx)
                }
            } else {
                workspace.show_error(format!("{error}"), cx)
            }
        }
    });

    Ok((
        workspace,
        items.into_iter().map(|item| item?.ok()).collect(),
    ))
}

fn deserialize_remote_project(
    connection_options: RemoteConnectionOptions,
    paths: Vec<PathBuf>,
    cx: &AsyncApp,
) -> Task<Result<(WorkspaceId, Option<SerializedWorkspace>)>> {
    let db = cx.update(|cx| WorkspaceDb::global(cx));
    cx.background_spawn(async move {
        let remote_connection_id = db
            .get_or_create_remote_connection(connection_options)
            .await?;

        let serialized_workspace = db.remote_workspace_for_roots(&paths, remote_connection_id);

        let workspace_id = if let Some(workspace_id) =
            serialized_workspace.as_ref().map(|workspace| workspace.id)
        {
            workspace_id
        } else {
            db.next_id().await?
        };

        Ok((workspace_id, serialized_workspace))
    })
}

pub fn activate_any_workspace_window(cx: &mut AsyncApp) -> Option<WindowHandle<MultiWorkspace>> {
    cx.update(|cx| {
        if let Some(workspace_window) = cx
            .active_window()
            .and_then(|window| window.downcast::<MultiWorkspace>())
        {
            return Some(workspace_window);
        }

        for window in cx.windows() {
            if let Some(workspace_window) = window.downcast::<MultiWorkspace>() {
                workspace_window
                    .update(cx, |_, window, _| window.activate_window())
                    .ok();
                return Some(workspace_window);
            }
        }
        None
    })
}

pub async fn get_any_active_multi_workspace(
    app_state: Arc<AppState>,
    mut cx: AsyncApp,
) -> anyhow::Result<WindowHandle<MultiWorkspace>> {
    // find an existing workspace to focus and show call controls
    let active_window = activate_any_workspace_window(&mut cx);
    if active_window.is_none() {
        cx.update(|cx| {
            Workspace::new_local(
                vec![],
                app_state.clone(),
                None,
                None,
                None,
                OpenMode::Activate,
                cx,
            )
        })
        .await?;
    }
    activate_any_workspace_window(&mut cx).context("could not open zed")
}

pub async fn restore_multiworkspace(
    multi_workspace: SerializedMultiWorkspace,
    app_state: Arc<AppState>,
    cx: &mut AsyncApp,
) -> anyhow::Result<WindowHandle<MultiWorkspace>> {
    let SerializedMultiWorkspace {
        active_workspace,
        state,
    } = multi_workspace;

    let workspace_result = if active_workspace.paths.is_empty() {
        cx.update(|cx| {
            open_workspace_by_id(active_workspace.workspace_id, app_state.clone(), None, cx)
        })
        .await
    } else {
        cx.update(|cx| {
            Workspace::new_local(
                active_workspace.paths.paths().to_vec(),
                app_state.clone(),
                None,
                None,
                None,
                OpenMode::Add,
                cx,
            )
        })
        .await
        .map(|result| result.window)
    };

    let window_handle = match workspace_result {
        Ok(handle) => {
            restore_native_window_state(handle, active_workspace.workspace_id, cx);
            handle
                .update(cx, |_, window, _cx| {
                    window.activate_window();
                })
                .ok();
            handle
        }
        Err(err) => {
            log::error!("Failed to restore active workspace: {err:#}");

            let mut fallback_handle = None;
            for key in &state.project_groups {
                let key: ProjectGroupKey = key.clone().into();
                let paths = key.path_list().paths().to_vec();
                match cx
                    .update(|cx| {
                        Workspace::new_local(
                            paths,
                            app_state.clone(),
                            None,
                            None,
                            None,
                            OpenMode::Activate,
                            cx,
                        )
                    })
                    .await
                {
                    Ok(OpenResult { window, .. }) => {
                        fallback_handle = Some(window);
                        break;
                    }
                    Err(fallback_err) => {
                        log::error!("Fallback project group also failed: {fallback_err:#}");
                    }
                }
            }

            fallback_handle.ok_or(err)?
        }
    };

    apply_restored_multiworkspace_state(window_handle, &state, app_state.fs.clone(), cx).await;

    window_handle
        .update(cx, |_, window, _cx| {
            window.activate_window();
        })
        .ok();

    Ok(window_handle)
}

pub async fn apply_restored_multiworkspace_state(
    window_handle: WindowHandle<MultiWorkspace>,
    state: &MultiWorkspaceState,
    fs: Arc<dyn fs::Fs>,
    cx: &mut AsyncApp,
) {
    let MultiWorkspaceState {
        sidebar_open,
        project_groups,
        sidebar_state,
        ..
    } = state;

    if !project_groups.is_empty() {
        // Resolve linked worktree paths to their main repo paths so
        // stale keys from previous sessions get normalized and deduped.
        let mut resolved_groups: Vec<SerializedProjectGroupState> = Vec::new();
        for serialized in project_groups.iter().cloned() {
            let SerializedProjectGroupState { key, expanded } = serialized.into_restored_state();
            if key.path_list().paths().is_empty() {
                continue;
            }
            let mut resolved_paths = Vec::new();
            for path in key.path_list().paths() {
                if key.host().is_none()
                    && let Some(common_dir) =
                        project::discover_root_repo_common_dir(path, fs.as_ref()).await
                    && !project::is_submodule_git_dir(&common_dir)
                {
                    let main_path = project::repo_identity_path(&common_dir, PathStyle::local());
                    resolved_paths.push(main_path.to_path_buf());
                } else {
                    resolved_paths.push(path.to_path_buf());
                }
            }
            let resolved = ProjectGroupKey::new(key.host(), PathList::new(&resolved_paths));
            if !resolved_groups.iter().any(|g| g.key == resolved) {
                resolved_groups.push(SerializedProjectGroupState {
                    key: resolved,
                    expanded,
                });
            }
        }

        window_handle
            .update(cx, |multi_workspace, _window, cx| {
                multi_workspace.restore_project_groups(resolved_groups, cx);
            })
            .ok();
    }

    if *sidebar_open {
        window_handle
            .update(cx, |multi_workspace, _, cx| {
                multi_workspace.restore_open_sidebar(cx);
            })
            .ok();
    }

    if let Some(sidebar_state) = sidebar_state {
        window_handle
            .update(cx, |multi_workspace, window, cx| {
                if let Some(sidebar) = multi_workspace.sidebar() {
                    sidebar.restore_serialized_state(sidebar_state, window, cx);
                }
                multi_workspace.serialize(cx);
            })
            .ok();
    }
}

fn restore_native_window_state(
    window_handle: WindowHandle<MultiWorkspace>,
    workspace_id: WorkspaceId,
    cx: &mut AsyncApp,
) {
    if window_bounds_env_override().is_some() {
        return;
    }
    let Some((Some(display), Some(native_window_state))) = cx
        .update(|cx| WorkspaceDb::global(cx))
        .native_window_state(workspace_id)
        .log_err()
        .flatten()
    else {
        return;
    };
    let display_connected = cx.update(|cx| {
        cx.displays()
            .into_iter()
            .any(|connected_display| connected_display.uuid().ok() == Some(display))
    });
    if !display_connected {
        return;
    }
    window_handle
        .update(cx, |_, window, _cx| {
            window.restore_native_window_state(&native_window_state);
        })
        .log_err();
}

pub async fn last_opened_workspace_location(
    db: &WorkspaceDb,
    fs: &dyn fs::Fs,
) -> Option<(WorkspaceId, SerializedWorkspaceLocation, PathList)> {
    db.last_workspace(fs)
        .await
        .log_err()
        .flatten()
        .map(|workspace| (workspace.workspace_id, workspace.location, workspace.paths))
}

pub async fn last_session_workspace_locations(
    db: &WorkspaceDb,
    last_session_id: &str,
    last_session_window_stack: Option<Vec<WindowId>>,
    fs: &dyn fs::Fs,
) -> Option<Vec<SessionWorkspace>> {
    db.last_session_workspace_locations(last_session_id, last_session_window_stack, fs)
        .await
        .log_err()
}

pub fn workspace_windows_for_location(
    serialized_location: &SerializedWorkspaceLocation,
    cx: &App,
) -> Vec<WindowHandle<MultiWorkspace>> {
    cx.windows()
        .into_iter()
        .filter_map(|window| window.downcast::<MultiWorkspace>())
        .filter(|multi_workspace| {
            let same_host = |left: &RemoteConnectionOptions, right: &RemoteConnectionOptions| match (left, right) {
                (RemoteConnectionOptions::Ssh(a), RemoteConnectionOptions::Ssh(b)) => {
                    (&a.host, &a.username, &a.port) == (&b.host, &b.username, &b.port)
                }
                (RemoteConnectionOptions::Wsl(a), RemoteConnectionOptions::Wsl(b)) => {
                    // The WSL username is not consistently populated in the workspace location, so ignore it for now.
                    a.distro_name == b.distro_name
                }
                (RemoteConnectionOptions::Docker(a), RemoteConnectionOptions::Docker(b)) => {
                    a.container_id == b.container_id
                }
                #[cfg(any(test, feature = "test-support"))]
                (RemoteConnectionOptions::Mock(a), RemoteConnectionOptions::Mock(b)) => {
                    a.id == b.id
                }
                _ => false,
            };

            multi_workspace.read(cx).is_ok_and(|multi_workspace| {
                multi_workspace.workspaces().any(|workspace| {
                    match workspace.read(cx).workspace_location(cx) {
                        WorkspaceLocation::Location(location, _) => {
                            match (&location, serialized_location) {
                                (
                                    SerializedWorkspaceLocation::Local,
                                    SerializedWorkspaceLocation::Local,
                                ) => true,
                                (
                                    SerializedWorkspaceLocation::Remote(a),
                                    SerializedWorkspaceLocation::Remote(b),
                                ) => same_host(a, b),
                                _ => false,
                            }
                        }
                        _ => false,
                    }
                })
            })
        })
        .collect()
}

pub async fn find_existing_workspace(
    abs_paths: &[PathBuf],
    open_options: &OpenOptions,
    location: &SerializedWorkspaceLocation,
    cx: &mut AsyncApp,
) -> (
    Option<(WindowHandle<MultiWorkspace>, Entity<Workspace>)>,
    OpenVisible,
) {
    let mut existing: Option<(WindowHandle<MultiWorkspace>, Entity<Workspace>)> = None;
    let mut open_visible = OpenVisible::All;
    let mut best_match = None;

    if open_options.workspace_matching != WorkspaceMatching::None {
        cx.update(|cx| {
            for window in workspace_windows_for_location(location, cx) {
                if let Ok(multi_workspace) = window.read(cx) {
                    for workspace in multi_workspace.workspaces() {
                        let project = workspace.read(cx).project.read(cx);
                        let m = match open_options.workspace_matching {
                            WorkspaceMatching::None => None,
                            WorkspaceMatching::MatchExact => {
                                project.visibility_for_paths(abs_paths, true, cx)
                            }
                            WorkspaceMatching::MatchSubpaths => {
                                project.visibility_for_subpaths(abs_paths, cx)
                            }
                            WorkspaceMatching::MatchSubdirectory => {
                                project.visibility_for_paths(abs_paths, false, cx)
                            }
                        };
                        if m > best_match {
                            existing = Some((window, workspace.clone()));
                            best_match = m;
                        } else if best_match.is_none()
                            && open_options.workspace_matching
                                == WorkspaceMatching::MatchSubdirectory
                        {
                            existing = Some((window, workspace.clone()))
                        }
                    }
                }
            }
        });

        let all_paths_are_files = existing
            .as_ref()
            .and_then(|(_, target_workspace)| {
                cx.update(|cx| {
                    let workspace = target_workspace.read(cx);
                    let project = workspace.project.read(cx);
                    let path_style = workspace.path_style(cx);
                    Some(!abs_paths.iter().any(|path| {
                        let path = util::paths::SanitizedPath::new(path);
                        project.worktrees(cx).any(|worktree| {
                            let worktree = worktree.read(cx);
                            let abs_path = worktree.abs_path();
                            path_style
                                .strip_prefix(path.as_ref(), abs_path.as_ref())
                                .and_then(|rel| worktree.entry_for_path(&rel))
                                .is_some_and(|e| e.is_dir())
                        })
                    }))
                })
            })
            .unwrap_or(false);

        if open_options.wait && existing.is_some() && all_paths_are_files {
            cx.update(|cx| {
                let windows = workspace_windows_for_location(location, cx);
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
    (existing, open_visible)
}

/// Controls whether to reuse an existing workspace whose worktrees contain the
/// given paths, and how broadly to match.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WorkspaceMatching {
    /// Always open a new workspace. No matching against existing worktrees.
    None,
    /// Match paths against existing worktree roots and files within them.
    #[default]
    MatchExact,
    /// Match files and directories inside existing worktrees, excluding the
    /// worktree roots themselves.
    MatchSubpaths,
    /// Match paths against existing worktrees including subdirectories, and
    /// fall back to any existing window if no worktree matched.
    ///
    /// For example, `zed -a foo/bar` will activate the `bar` workspace if it
    /// exists, otherwise it will open a new window with `foo/bar` as the root.
    MatchSubdirectory,
}

#[derive(Clone)]
pub struct OpenOptions {
    pub visible: Option<OpenVisible>,
    pub focus: Option<bool>,
    pub workspace_matching: WorkspaceMatching,
    /// Whether to add unmatched directories to the existing window's sidebar
    /// rather than opening a new window. Defaults to true, matching the default
    /// `cli_default_open_behavior` setting.
    pub add_dirs_to_sidebar: bool,
    pub wait: bool,
    pub requesting_window: Option<WindowHandle<MultiWorkspace>>,
    pub open_mode: OpenMode,
    pub env: Option<HashMap<String, String>>,
    pub open_in_dev_container: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            visible: None,
            focus: None,
            workspace_matching: WorkspaceMatching::default(),
            add_dirs_to_sidebar: true,
            wait: false,
            requesting_window: None,
            open_mode: OpenMode::default(),
            env: None,
            open_in_dev_container: false,
        }
    }
}

impl OpenOptions {
    fn should_reuse_existing_window(&self) -> bool {
        !matches!(
            self.workspace_matching,
            WorkspaceMatching::None | WorkspaceMatching::MatchSubpaths
        ) && self.open_mode != OpenMode::NewWindow
    }
}

/// The result of opening a workspace via [`open_paths`], [`Workspace::new_local`],
/// or [`Workspace::open_workspace_for_paths`].
pub struct OpenResult {
    pub window: WindowHandle<MultiWorkspace>,
    pub workspace: Entity<Workspace>,
    pub opened_items: Vec<Option<anyhow::Result<Box<dyn ItemHandle>>>>,
}

#[derive(Debug)]
pub struct WorkspacePosition {
    pub window_bounds: Option<WindowBounds>,
    pub display: Option<Uuid>,
    pub centered_layout: bool,
}

pub fn remote_workspace_position_from_db(
    connection_options: RemoteConnectionOptions,
    paths_to_open: &[PathBuf],
    cx: &App,
) -> Task<Result<WorkspacePosition>> {
    let paths = paths_to_open.to_vec();
    let db = WorkspaceDb::global(cx);
    let kvp = db::kvp::KeyValueStore::global(cx);

    cx.background_spawn(async move {
        let remote_connection_id = db
            .get_or_create_remote_connection(connection_options)
            .await
            .context("fetching serialized ssh project")?;
        let serialized_workspace = db.remote_workspace_for_roots(&paths, remote_connection_id);

        let (window_bounds, display) = if let Some(bounds) = window_bounds_env_override() {
            (Some(WindowBounds::Windowed(bounds)), None)
        } else {
            let restorable_bounds = serialized_workspace
                .as_ref()
                .and_then(|workspace| {
                    Some((workspace.display?, workspace.window_bounds.map(|b| b.0)?))
                })
                .or_else(|| persistence::read_default_window_bounds(&kvp));

            if let Some((serialized_display, serialized_bounds)) = restorable_bounds {
                (Some(serialized_bounds), Some(serialized_display))
            } else {
                (None, None)
            }
        };

        let centered_layout = serialized_workspace
            .as_ref()
            .map(|w| w.centered_layout)
            .unwrap_or(false);

        Ok(WorkspacePosition {
            window_bounds,
            display,
            centered_layout,
        })
    })
}
