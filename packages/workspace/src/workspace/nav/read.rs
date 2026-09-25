impl Workspace {
    //
    pub fn recently_activated_items(&self, cx: &App) -> HashMap<EntityId, usize> {
        let mut history: HashMap<EntityId, usize> = HashMap::default();

        for pane_handle in &self.panes {
            let pane = pane_handle.read(cx);

            for entry in pane.activation_history() {
                history.insert(
                    entry.entity_id,
                    history
                        .get(&entry.entity_id)
                        .cloned()
                        .unwrap_or(0)
                        .max(entry.timestamp),
                );
            }
        }

        history
    }
    //
    pub fn recent_active_item_by_type<T: 'static>(&self, cx: &App) -> Option<Entity<T>> {
        let mut recent_item: Option<Entity<T>> = None;
        let mut recent_timestamp = 0;
        for pane_handle in &self.panes {
            let pane = pane_handle.read(cx);
            let item_map: HashMap<EntityId, &Box<dyn ItemHandle>> =
                pane.items().map(|item| (item.item_id(), item)).collect();
            for entry in pane.activation_history() {
                if entry.timestamp > recent_timestamp
                    && let Some(&item) = item_map.get(&entry.entity_id)
                    && let Some(typed_item) = item.act_as::<T>(cx)
                {
                    recent_timestamp = entry.timestamp;
                    recent_item = Some(typed_item);
                }
            }
        }
        recent_item
    }
    //
    pub fn recent_navigation_history_iter(
        &self,
        cx: &App,
    ) -> impl Iterator<Item = (ProjectPath, Option<PathBuf>)> + use<> {
        let mut abs_paths_opened: HashMap<PathBuf, HashSet<ProjectPath>> = HashMap::default();
        let mut history: HashMap<ProjectPath, (Option<PathBuf>, usize)> = HashMap::default();

        for pane in &self.panes {
            let pane = pane.read(cx);

            pane.nav_history()
                .for_each_entry(cx, &mut |entry, (project_path, fs_path)| {
                    if let Some(fs_path) = &fs_path {
                        abs_paths_opened
                            .entry(fs_path.clone())
                            .or_default()
                            .insert(project_path.clone());
                    }
                    let timestamp = entry.timestamp;
                    match history.entry(project_path) {
                        hash_map::Entry::Occupied(mut entry) => {
                            let (_, old_timestamp) = entry.get();
                            if &timestamp > old_timestamp {
                                entry.insert((fs_path, timestamp));
                            }
                        }
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert((fs_path, timestamp));
                        }
                    }
                });

            if let Some(item) = pane.active_item()
                && let Some(project_path) = item.project_path(cx)
            {
                let fs_path = self.project.read(cx).absolute_path(&project_path, cx);

                if let Some(fs_path) = &fs_path {
                    abs_paths_opened
                        .entry(fs_path.clone())
                        .or_default()
                        .insert(project_path.clone());
                }

                history.insert(project_path, (fs_path, std::usize::MAX));
            }
        }

        let mut recent_history = history
            .into_iter()
            .sorted_by_key(|(_, (_, order))| *order)
            .map(|(project_path, (fs_path, _))| (project_path, fs_path))
            .rev()
            .filter(move |(history_path, abs_path)| {
                let latest_project_path_opened = abs_path
                    .as_ref()
                    .and_then(|abs_path| abs_paths_opened.get(abs_path))
                    .and_then(|project_paths| {
                        project_paths
                            .iter()
                            .max_by(|b1, b2| b1.worktree_id.cmp(&b2.worktree_id))
                    });

                latest_project_path_opened.is_none_or(|path| path == history_path)
            })
            .collect::<Vec<_>>();

        let mut seen_paths = recent_history
            .iter()
            .map(|(project_path, _)| project_path.clone())
            .collect::<HashSet<_>>();
        let project = self.project.read(cx);
        for abs_path in &self.persisted_recent_navigation_history {
            let Some(project_path) = project.project_path_for_absolute_path(abs_path, cx) else {
                continue;
            };
            if seen_paths.insert(project_path.clone()) {
                recent_history.push((project_path, Some(abs_path.clone())));
            }
        }

        recent_history.into_iter()
    }
    //
    pub fn recent_navigation_history(
        &self,
        limit: Option<usize>,
        cx: &App,
    ) -> Vec<(ProjectPath, Option<PathBuf>)> {
        self.recent_navigation_history_iter(cx)
            .take(limit.unwrap_or(usize::MAX))
            .collect()
    }
    // pub fn most_recent_active_path
    pub fn most_recent_active_path(&self, cx: &App) -> Option<PathBuf> {
        self.recent_navigation_history_iter(cx)
            .filter_map(|(path, abs_path)| {
                let worktree = self
                    .project
                    .read(cx)
                    .worktree_for_id(path.worktree_id, cx)?;
                if !worktree.read(cx).is_visible() {
                    return None;
                }
                let settings_location = SettingsLocation {
                    worktree_id: path.worktree_id,
                    path: &path.path,
                };
                if WorktreeSettings::get(Some(settings_location), cx).is_path_read_only(&path.path)
                {
                    return None;
                }
                abs_path
            })
            .next()
    }



}
