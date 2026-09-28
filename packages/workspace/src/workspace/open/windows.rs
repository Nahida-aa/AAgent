use super::*;
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

pub fn workspace_windows_for_location(
    serialized_location: &SerializedWorkspaceLocation,
    cx: &App,
) -> Vec<WindowHandle<MultiWorkspace>> {
    let all_windows = cx.windows();
    tracing::info!("workspace_windows_for_location: {} windows total", all_windows.len());
    all_windows
        .into_iter()
        .filter_map(|window| {
            let downcasted = window.downcast::<MultiWorkspace>();
            tracing::info!("  downcast: {}", downcasted.is_some());
            downcasted
        })
        .filter(|multi_workspace| {
            let read_result = multi_workspace.read(cx);
            tracing::info!("  multi_workspace.read: is_ok={}", read_result.is_ok());
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

            read_result.is_ok_and(|multi_workspace| {
                let ws_count = multi_workspace.workspaces().count();
                tracing::info!("  workspaces count: {}", ws_count);
                multi_workspace.workspaces().any(|workspace| {
                    let loc = workspace.read(cx).workspace_location(cx);
                    tracing::info!("    workspace location: {:?}", std::mem::discriminant(&loc));
                    match loc {
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
