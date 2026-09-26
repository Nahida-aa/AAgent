use super::*;
impl Workspace {
    //
    pub(crate) fn update_window_title(&mut self, window: &mut Window, cx: &mut App) {
        if !self.owns_window_chrome() {
            return;
        }
        self.apply_window_title(window, cx);
    }
    //
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

}
