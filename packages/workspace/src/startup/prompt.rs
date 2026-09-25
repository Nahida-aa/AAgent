pub fn init(app_state: Arc<AppState>, cx: &mut App) {
    component::init();
    theme_preview::init(cx);
    toast_layer::init(cx);
    history_manager::init(app_state.fs.clone(), cx);

    cx.on_app_quit(flush_windows_serialization_on_quit).detach();

    cx.on_action(|_: &CloseWindow, cx| Workspace::close_global(cx))
        .on_action(|_: &Reload, cx| reload(cx))
        .on_action(|action: &Open, cx: &mut App| {
            let app_state = AppState::global(cx);
            prompt_and_open_paths(
                app_state,
                PathPromptOptions {
                    files: true,
                    directories: true,
                    multiple: true,
                    prompt: None,
                },
                action.create_new_window.unwrap_or_else(|| {
                    matches!(
                        WorkspaceSettings::get_global(cx).default_open_behavior,
                        DefaultOpenBehavior::NewWindow
                    )
                }),
                cx,
            );
        })
        .on_action(|_: &OpenFiles, cx: &mut App| {
            let directories = cx.can_select_mixed_files_and_dirs();
            let app_state = AppState::global(cx);
            prompt_and_open_paths(
                app_state,
                PathPromptOptions {
                    files: true,
                    directories,
                    multiple: true,
                    prompt: None,
                },
                true,
                cx,
            );
        });
}

fn prompt_and_open_paths(
    app_state: Arc<AppState>,
    options: PathPromptOptions,
    create_new_window: bool,
    cx: &mut App,
) {
    if let Some(workspace_window) =
        workspace_windows_for_location(&SerializedWorkspaceLocation::Local, cx)
            .into_iter()
            .next()
    {
        workspace_window
            .update(cx, |multi_workspace, window, cx| {
                let workspace = multi_workspace.workspace().clone();
                workspace.update(cx, |workspace, cx| {
                    prompt_for_open_path_and_open(
                        workspace,
                        app_state,
                        options,
                        create_new_window,
                        window,
                        cx,
                    );
                });
            })
            .ok();
    } else {
        let task = Workspace::new_local(
            Vec::new(),
            app_state.clone(),
            None,
            None,
            None,
            OpenMode::Activate,
            cx,
        );
        cx.spawn(async move |cx| {
            let OpenResult { window, .. } = task.await?;
            window.update(cx, |multi_workspace, window, cx| {
                window.activate_window();
                let workspace = multi_workspace.workspace().clone();
                workspace.update(cx, |workspace, cx| {
                    prompt_for_open_path_and_open(
                        workspace,
                        app_state,
                        options,
                        create_new_window,
                        window,
                        cx,
                    );
                });
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}

pub fn prompt_for_open_path_and_open(
    workspace: &mut Workspace,
    app_state: Arc<AppState>,
    options: PathPromptOptions,
    create_new_window: bool,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let paths = workspace.prompt_for_open_path(
        options,
        DirectoryLister::Local(workspace.project().clone(), app_state.fs.clone()),
        window,
        cx,
    );
    let multi_workspace_handle = window.window_handle().downcast::<MultiWorkspace>();
    cx.spawn_in(window, async move |this, cx| {
        let Some(paths) = paths.await.log_err().flatten() else {
            return;
        };
        if !create_new_window {
            if let Some(handle) = multi_workspace_handle {
                if let Some(task) = handle
                    .update(cx, |multi_workspace, window, cx| {
                        multi_workspace.open_project(paths, OpenMode::Activate, window, cx)
                    })
                    .log_err()
                {
                    task.await.log_err();
                }
                return;
            }
        }
        if let Some(task) = this
            .update_in(cx, |this, window, cx| {
                this.open_workspace_for_paths(OpenMode::NewWindow, paths, window, cx)
            })
            .log_err()
        {
            task.await.log_err();
        }
    })
    .detach();
}
