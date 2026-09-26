use super::*;
#[derive(Default)]
pub(crate) struct WindowTitleContext {
    pub project_name: String,
    pub file_name: Option<String>,
    pub file_path: Option<String>,
    pub relative_path: Option<String>,
    pub file_stem: Option<String>,
    pub remote_name: Option<String>,
    pub remote_host: Option<String>,
    pub app_name: &'static str,
    pub branch: Option<String>,
}

impl WindowTitleContext {
    fn value_for(&self, variable: &str) -> Option<&str> {
        match variable {
            "projectName" => Some(self.project_name.as_str()),
            "fileName" => self.file_name.as_deref(),
            "filePath" => self.file_path.as_deref(),
            "relativePath" => self.relative_path.as_deref(),
            "fileStem" => self.file_stem.as_deref(),
            "remoteName" => self.remote_name.as_deref(),
            "remoteHost" => self.remote_host.as_deref(),
            "appName" => Some(self.app_name),
            "branch" => self.branch.as_deref(),
            // Unknown placeholders collapse like missing values so imported and
            // native templates follow the same rendering rules.
            _ => None,
        }
    }
}

impl Workspace {
    //
    fn window_title_context(
        project: &Project,
        project_path: Option<&ProjectPath>,
        needs: &WindowTitleNeeds,
        cx: &App,
    ) -> WindowTitleContext {
        let project_name = project_window_title(project, cx);
        let path_style = project.path_style(cx);

        let (file_name, file_path, relative_path, file_stem) = project_path
            .map(|project_path| {
                let file_name = project_path
                    .path
                    .file_name()
                    .map(|file_name| file_name.to_string())
                    .or_else(|| {
                        Some(
                            project
                                .worktree_for_id(project_path.worktree_id, cx)?
                                .read(cx)
                                .root_name_str()
                                .to_string(),
                        )
                    });
                let file_path = if needs.file_path {
                    project
                        .absolute_path(project_path, cx)
                        .map(|path| path.to_string_lossy().into_owned())
                } else {
                    None
                };
                let relative_path = if needs.relative_path {
                    (!project_path.path.as_unix_str().is_empty())
                        .then(|| project_path.path.display(path_style).to_string())
                } else {
                    None
                };
                let file_stem = if needs.file_stem {
                    project_path.path.file_stem().map(|s| s.to_string())
                } else {
                    None
                };
                (file_name, file_path, relative_path, file_stem)
            })
            .unwrap_or((None, None, None, None));

        let remote_options = if needs.remote {
            project.remote_connection_options(cx)
        } else {
            None
        };
        let remote_name = remote_options
            .as_ref()
            .map(RemoteConnectionOptions::display_name);
        let remote_host = remote_options.as_ref().map(RemoteConnectionOptions::host);

        let branch = if needs.branch {
            project
                .active_repository(cx)
                .and_then(|repo| repo.read(cx).branch.as_ref().map(|b| b.name().to_owned()))
        } else {
            None
        };

        WindowTitleContext {
            project_name,
            file_name,
            file_path,
            relative_path,
            file_stem,
            remote_name,
            remote_host,
            app_name: if needs.app_name {
                ReleaseChannel::try_global(cx)
                    .unwrap_or(ReleaseChannel::Stable)
                    .display_name()
            } else {
                ""
            },
            branch,
        }
    }
}
//
pub(crate) fn project_window_title(project: &Project, cx: &App) -> String {
    let mut title = String::new();

    for (index, worktree) in project.visible_worktrees(cx).enumerate() {
        let name = worktree.read(cx).root_name_str();
        if index > 0 {
            title.push_str(", ");
        }
        title.push_str(name);
    }

    if title.is_empty() {
        // Keep the default untitled-window text instead of showing a blank title.
        "empty project".to_string()
    } else {
        title
    }
}
