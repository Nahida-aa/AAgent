/// Which optional window-title variables are actually referenced by the active
/// template. Used to skip expensive lookups when the template doesn't need them.
pub(crate) struct WindowTitleNeeds {
    pub file_path: bool,
    pub relative_path: bool,
    pub file_stem: bool,
    pub remote: bool,
    pub app_name: bool,
    pub branch: bool,
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

impl Workspace {


    /// Whether the active window-title template references `${branch}`, and so
    /// can be affected by Git repository events.
    pub(crate) fn window_title_needs_branch(&self, cx: &App) -> bool {
        WindowTitleNeeds::from_template(&WorkspaceSettings::get_global(cx).window_title_format)
            .branch
    }


}
