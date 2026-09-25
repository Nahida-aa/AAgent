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
