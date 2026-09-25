pub fn create_and_open_local_file(
    path: &'static Path,
    window: &mut Window,
    cx: &mut Context<Workspace>,
    default_content: impl 'static + Send + FnOnce() -> Rope,
) -> Task<Result<Box<dyn ItemHandle>>> {
    cx.spawn_in(window, async move |workspace, cx| {
        let fs = workspace.read_with(cx, |workspace, _| workspace.app_state().fs.clone())?;
        if !fs.is_file(path).await {
            fs.create_file(path, Default::default()).await?;
            fs.save(path, &default_content(), Default::default())
                .await?;
        }

        workspace
            .update_in(cx, |workspace, window, cx| {
                workspace.with_local_or_wsl_workspace(window, cx, |workspace, window, cx| {
                    let path = workspace
                        .project
                        .read_with(cx, |project, cx| project.try_windows_path_to_wsl(path, cx));
                    cx.spawn_in(window, async move |workspace, cx| {
                        let path = path.await?;

                        let path = fs.canonicalize(&path).await.unwrap_or(path);

                        let mut items = workspace
                            .update_in(cx, |workspace, window, cx| {
                                workspace.open_paths(
                                    vec![path.to_path_buf()],
                                    OpenOptions {
                                        visible: Some(OpenVisible::None),
                                        ..Default::default()
                                    },
                                    None,
                                    window,
                                    cx,
                                )
                            })?
                            .await;
                        let item = items.pop().flatten();
                        item.with_context(|| format!("path {path:?} is not a file"))?
                    })
                })
            })?
            .await?
            .await
    })
}
