pub(crate) fn prompt_and_open_paths(
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

pub(crate) type PromptForNewPath = Box<
    dyn Fn(
        &mut Workspace,
        DirectoryLister,
        Option<String>,
        &mut Window,
        &mut Context<Workspace>,
    ) -> oneshot::Receiver<Option<Vec<PathBuf>>>,
>;

pub(crate) type PromptForOpenPath = Box<
    dyn Fn(
        &mut Workspace,
        DirectoryLister,
        &mut Window,
        &mut Context<Workspace>,
    ) -> oneshot::Receiver<Option<Vec<PathBuf>>>,
>;

impl Workspace {
    // pub fn set_prompt_for_new_path
    pub fn set_prompt_for_new_path(&mut self, prompt: PromptForNewPath) {
        self.on_prompt_for_new_path = Some(prompt)
    }
    // pub fn set_prompt_for_open_path
    pub fn set_prompt_for_open_path(&mut self, prompt: PromptForOpenPath) {
        self.on_prompt_for_open_path = Some(prompt)
    }
    // pub fn prompt_for_open_path
    pub fn prompt_for_open_path(
        &mut self,
        path_prompt_options: PathPromptOptions,
        lister: DirectoryLister,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> oneshot::Receiver<Option<Vec<PathBuf>>> {
        // TODO: If `on_prompt_for_open_path` is set, we should always use it
        // rather than gating on `use_system_path_prompts`. This would let tests
        // inject a mock without also having to disable the setting.
        if !lister.is_local(cx) || !WorkspaceSettings::get_global(cx).use_system_path_prompts {
            let prompt = self.on_prompt_for_open_path.take().unwrap();
            let rx = prompt(self, lister, window, cx);
            self.on_prompt_for_open_path = Some(prompt);
            rx
        } else {
            let (tx, rx) = oneshot::channel();
            let abs_path = cx.prompt_for_paths(path_prompt_options);

            cx.spawn_in(window, async move |workspace, cx| {
                let Ok(result) = abs_path.await else {
                    return Ok(());
                };

                match result {
                    Ok(result) => {
                        tx.send(result).ok();
                    }
                    Err(err) => {
                        let rx = workspace.update_in(cx, |workspace, window, cx| {
                            workspace
                                .show_error(workspace_error::PortalError::new(err.to_string()), cx);
                            let prompt = workspace.on_prompt_for_open_path.take().unwrap();
                            let rx = prompt(workspace, lister, window, cx);
                            workspace.on_prompt_for_open_path = Some(prompt);
                            rx
                        })?;
                        if let Ok(path) = rx.await {
                            tx.send(path).ok();
                        }
                    }
                };
                anyhow::Ok(())
            })
            .detach();

            rx
        }
    }
    // pub fn prompt_for_new_path
    pub fn prompt_for_new_path(
        &mut self,
        lister: DirectoryLister,
        suggested_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> oneshot::Receiver<Option<Vec<PathBuf>>> {
        if self.project.read(cx).is_via_collab()
            || self.project.read(cx).is_via_remote_server()
            || !WorkspaceSettings::get_global(cx).use_system_path_prompts
        {
            let prompt = self.on_prompt_for_new_path.take().unwrap();
            let rx = prompt(self, lister, suggested_name, window, cx);
            self.on_prompt_for_new_path = Some(prompt);
            return rx;
        }

        let (tx, rx) = oneshot::channel();
        cx.spawn_in(window, async move |workspace, cx| {
            let abs_path = workspace.update(cx, |workspace, cx| {
                let relative_to = workspace
                    .most_recent_active_path(cx)
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .or_else(|| {
                        let project = workspace.project.read(cx);
                        project.visible_worktrees(cx).find_map(|worktree| {
                            Some(worktree.read(cx).as_local()?.abs_path().to_path_buf())
                        })
                    })
                    .or_else(std::env::home_dir)
                    .unwrap_or_else(|| PathBuf::from(""));
                cx.prompt_for_new_path(&relative_to, suggested_name.as_deref())
            })?;
            let abs_path = match abs_path.await? {
                Ok(path) => path,
                Err(err) => {
                    let rx = workspace.update_in(cx, |workspace, window, cx| {
                        workspace
                            .show_error(workspace_error::PortalError::new(err.to_string()), cx);

                        let prompt = workspace.on_prompt_for_new_path.take().unwrap();
                        let rx = prompt(workspace, lister, suggested_name, window, cx);
                        workspace.on_prompt_for_new_path = Some(prompt);
                        rx
                    })?;
                    if let Ok(path) = rx.await {
                        tx.send(path).ok();
                    }
                    return anyhow::Ok(());
                }
            };

            tx.send(abs_path.map(|path| vec![path])).ok();
            anyhow::Ok(())
        })
        .detach();

        rx
    }
}
