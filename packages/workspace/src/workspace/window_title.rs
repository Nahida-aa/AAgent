use gpui::{App, Context, Entity};

use super::Workspace;
use crate::dock::Dock;

/// Which optional window-title variables are actually referenced by the active
/// template. Used to skip expensive lookups when the template doesn't need them.
struct WindowTitleNeeds {
    file_path: bool,
    relative_path: bool,
    file_stem: bool,
    remote: bool,
    app_name: bool,
    branch: bool,
}

impl WindowTitleNeeds {
    fn from_template(template: &str) -> Self {
        Self {
            file_path: template.contains("${filePath}"),
            relative_path: template.contains("${relativePath}"),
            file_stem: template.contains("${fileStem}"),
            remote: template.contains("${remoteName}") || template.contains("${remoteHost}"),
            app_name: template.contains("${appName}"),
            branch: template.contains("${branch}"),
        }
    }
}

#[derive(Default)]
struct WindowTitleContext {
    project_name: String,
    file_name: Option<String>,
    file_path: Option<String>,
    relative_path: Option<String>,
    file_stem: Option<String>,
    remote_name: Option<String>,
    remote_host: Option<String>,
    app_name: &'static str,
    branch: Option<String>,
}

enum WindowTitleTemplatePart<'a> {
    Literal(&'a str),
    Variable(&'a str),
    Separator,
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

fn parse_window_title_format(template: &str) -> Vec<WindowTitleTemplatePart<'_>> {
    let mut parts = Vec::new();
    let mut start = 0;

    // Keep this placeholder scan in sync with the importer in
    // settings/src/vscode_import.rs.
    while let Some(offset) = template[start..].find("${") {
        let variable_start = start + offset;
        if variable_start > start {
            parts.push(WindowTitleTemplatePart::Literal(
                &template[start..variable_start],
            ));
        }

        let content_start = variable_start + 2;
        let Some(content_end_offset) = template[content_start..].find('}') else {
            parts.push(WindowTitleTemplatePart::Literal(
                &template[variable_start..],
            ));
            return parts;
        };

        let content_end = content_start + content_end_offset;
        let variable = &template[content_start..content_end];
        if variable == "separator" {
            parts.push(WindowTitleTemplatePart::Separator);
        } else {
            parts.push(WindowTitleTemplatePart::Variable(variable));
        }

        start = content_end + 1;
    }

    if start < template.len() {
        parts.push(WindowTitleTemplatePart::Literal(&template[start..]));
    }

    parts
}

fn render_window_title_format(
    template: &str,
    separator: &str,
    context: &WindowTitleContext,
) -> String {
    let parts = parse_window_title_format(template);
    let mut segments = Vec::new();
    let mut current_segment = String::new();

    for part in parts {
        match part {
            WindowTitleTemplatePart::Literal(text) => current_segment.push_str(text),
            WindowTitleTemplatePart::Variable(variable) => {
                if let Some(value) = context.value_for(variable) {
                    current_segment.push_str(value);
                }
            }
            WindowTitleTemplatePart::Separator => {
                if !current_segment.is_empty() {
                    segments.push(std::mem::take(&mut current_segment));
                }
            }
        }
    }

    if !current_segment.is_empty() {
        segments.push(current_segment);
    }

    segments.join(separator)
}

impl Workspace {
    fn update_window_title(&mut self, window: &mut Window, cx: &mut App) {
        if !self.owns_window_chrome() {
            return;
        }
        self.apply_window_title(window, cx);
    }
    fn apply_window_title(&mut self, window: &mut Window, cx: &mut App) {
        let project = self.project().read(cx);
        let active_project_path = self.active_item(cx).and_then(|item| item.project_path(cx));
        let settings = WorkspaceSettings::get_global(cx);
        let template = settings.window_title_format.as_str();
        let separator = settings.window_title_separator.as_str();
        let settings_changed = self.last_window_title_settings.as_ref().is_none_or(
            |(last_template, last_separator)| {
                (last_template.as_str(), last_separator.as_str()) != (template, separator)
            },
        );
        if settings_changed {
            self.last_window_title_settings = Some((template.to_string(), separator.to_string()));
        }
        let needs = WindowTitleNeeds::from_template(template);
        let context =
            Self::window_title_context(&project, active_project_path.as_ref(), &needs, cx);
        let mut title = render_window_title_format(template, separator, &context);
        // Keep the normal title when a custom template resolves entirely to
        // empty or unknown placeholders.
        if title.trim().is_empty()
            && let Some(default_template) = cx
                .global::<SettingsStore>()
                .raw_default_settings()
                .workspace
                .window_title_format
                .as_deref()
        {
            let needs = WindowTitleNeeds::from_template(default_template);
            let context =
                Self::window_title_context(&project, active_project_path.as_ref(), &needs, cx);
            title = render_window_title_format(default_template, separator, &context);
        }

        if project.is_via_collab() {
            title.push_str(" ↙");
        } else if project.is_shared() {
            title.push_str(" ↗");
        }

        let document_path = active_project_path
            .as_ref()
            .and_then(|path| project.absolute_path(path, cx));
        window.set_document_path(document_path.as_deref());

        if let Some(last_title) = self.last_window_title.as_ref()
            && &title == last_title
        {
            return;
        }
        window.set_window_title(&title);
        SystemWindowTabController::update_tab_title(
            cx,
            window.window_handle().window_id(),
            SharedString::from(&title),
        );
        self.last_window_title = Some(title);
    }

    /// Whether the active window-title template references `${branch}`, and so
    /// can be affected by Git repository events.
    fn window_title_needs_branch(&self, cx: &App) -> bool {
        WindowTitleNeeds::from_template(&WorkspaceSettings::get_global(cx).window_title_format)
            .branch
    }

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

    pub(super) fn update_window_edited(&mut self, window: &mut Window, cx: &mut App) {
        if !self.owns_window_chrome() {
            return;
        }
        let is_edited = self.is_window_edited(cx);
        if is_edited != self.window_edited {
            self.window_edited = is_edited;
            window.set_window_edited(self.window_edited);
        }
    }

    fn update_item_dirty_state(
        &mut self,
        item: &dyn ItemHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        let is_dirty = item.is_dirty(cx);
        let item_id = item.item_id();
        let was_dirty = self.dirty_items.contains_key(&item_id);
        if is_dirty == was_dirty {
            return;
        }
        if was_dirty {
            self.dirty_items.remove(&item_id);
            self.update_window_edited(window, cx);
            return;
        }

        let workspace = self.weak_handle();
        let Some(window_handle) = window.window_handle().downcast::<MultiWorkspace>() else {
            return;
        };
        let on_release_callback = Box::new(move |cx: &mut App| {
            window_handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |workspace, cx| {
                            workspace.dirty_items.remove(&item_id);
                            workspace.update_window_edited(window, cx)
                        })
                        .ok();
                })
                .ok();
        });

        let s = item.on_release(cx, on_release_callback);
        self.dirty_items.insert(item_id, s);
        self.update_window_edited(window, cx);
    }

    fn update_window_edited(&mut self, window: &mut Window, cx: &mut App) {
        if !self.owns_window_chrome() {
            return;
        }
        let is_edited = self.is_window_edited(cx);
        if is_edited != self.window_edited {
            self.window_edited = is_edited;
            window.set_window_edited(self.window_edited);
        }
    }
}
