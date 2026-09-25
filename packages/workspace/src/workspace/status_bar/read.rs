impl Workspace {
    // status_bar
    pub fn status_bar(&self) -> &Entity<StatusBar> { &self.status_bar }
    // status_bar_visible
    pub fn status_bar_visible(&self, cx: &App) -> bool { StatusBarSettings::get_global(cx).show }
}
