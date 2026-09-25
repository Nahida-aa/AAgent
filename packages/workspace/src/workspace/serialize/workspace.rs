enum WorkspaceLocation {
    // Valid local paths or SSH project to serialize
    Location(SerializedWorkspaceLocation, PathList),
    // No valid location found to serialize
    None,
}

impl WorkspaceLocation {
    pub(crate) fn workspace_location(&self, cx: &App) -> WorkspaceLocation {
        let paths = PathList::new(&self.root_paths(cx));
        if let Some(connection) = self.project.read(cx).remote_connection_options(cx) {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Remote(connection), paths)
        } else if self.project.read(cx).is_local() {
            WorkspaceLocation::Location(SerializedWorkspaceLocation::Local, paths)
        } else {
            WorkspaceLocation::None
        }
    }

}
