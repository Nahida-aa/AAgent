use collections::TypeIdHashMap;

type BuildProjectItemFn =
    fn(AnyEntity, Entity<Project>, Option<&Pane>, &mut Window, &mut App) -> Box<dyn ItemHandle>;

    type WorkspaceItemBuilder =
        Box<dyn FnOnce(&mut Pane, &mut Window, &mut Context<Pane>) -> Box<dyn ItemHandle>>;

type BuildProjectItemForPathFn =
    fn(
        &Entity<Project>,
        &ProjectPath,
        &mut Window,
        &mut App,
    ) -> Option<Task<Result<(Option<ProjectEntryId>, WorkspaceItemBuilder)>>>;

#[derive(Clone, Default)]
struct ProjectItemRegistry {
    build_project_item_fns_by_type: TypeIdHashMapp<BuildProjectItemFn>,
    build_project_item_for_path_fns: Vec<BuildProjectItemForPathFn>,
}

impl ProjectItemRegistry {
    fn register<T: ProjectItem>(&mut self) {
        self.build_project_item_fns_by_type.insert(
            TypeId::of::<T::Item>(),
            |item, project, pane, window, cx| {
                let item = item.downcast().unwrap();
                Box::new(cx.new(|cx| T::for_project_item(project, pane, item, window, cx)))
                    as Box<dyn ItemHandle>
            },
        );
        self.build_project_item_for_path_fns
            .push(|project, project_path, window, cx| {
                let project_path = project_path.clone();
                let is_file = project
                    .read(cx)
                    .entry_for_path(&project_path, cx)
                    .is_some_and(|entry| entry.is_file());
                let entry_abs_path = project.read(cx).absolute_path(&project_path, cx);
                let is_local = project.read(cx).is_local();
                let project_item =
                    <T::Item as project::ProjectItem>::try_open(project, &project_path, cx)?;
                let project = project.clone();
                Some(window.spawn(cx, async move |cx| {
                    match project_item.await.with_context(|| {
                        format!(
                            "opening project path {:?}",
                            entry_abs_path.as_deref().unwrap_or(&project_path.path.as_std_path())
                        )
                    }) {
                        Ok(project_item) => {
                            let project_item = project_item;
                            let project_entry_id: Option<ProjectEntryId> =
                                project_item.read_with(cx, project::ProjectItem::entry_id);
                            let build_workspace_item = Box::new(
                                |pane: &mut Pane, window: &mut Window, cx: &mut Context<Pane>| {
                                    Box::new(cx.new(|cx| {
                                        T::for_project_item(
                                            project,
                                            Some(pane),
                                            project_item,
                                            window,
                                            cx,
                                        )
                                    })) as Box<dyn ItemHandle>
                                },
                            ) as Box<_>;
                            Ok((project_entry_id, build_workspace_item))
                        }
                        Err(e) => {
                            log::warn!("Failed to open a project item: {e:#}");
                            if e.error_code() == ErrorCode::Internal {
                                if let Some(abs_path) =
                                    entry_abs_path.as_deref().filter(|_| is_file)
                                {
                                    if let Some(broken_project_item_view) =
                                        cx.update(|window, cx| {
                                            T::for_broken_project_item(
                                                abs_path, is_local, &e, window, cx,
                                            )
                                        })?
                                    {
                                        let build_workspace_item = Box::new(
                                            move |_: &mut Pane, _: &mut Window, cx: &mut Context<Pane>| {
                                                cx.new(|_| broken_project_item_view).boxed_clone()
                                            },
                                        )
                                        as Box<_>;
                                        return Ok((None, build_workspace_item));
                                    }
                                }
                            }
                            Err(e)
                        }
                    }
                }))
            });
    }

    fn open_path(
        &self,
        project: &Entity<Project>,
        path: &ProjectPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<(Option<ProjectEntryId>, WorkspaceItemBuilder)>> {
        let Some(open_project_item) = self
            .build_project_item_for_path_fns
            .iter()
            .rev()
            .find_map(|open_project_item| open_project_item(project, path, window, cx))
        else {
            return Task::ready(Err(anyhow!("cannot open file {:?}", path.path)));
        };
        open_project_item
    }

    fn build_item<T: project::ProjectItem>(
        &self,
        item: Entity<T>,
        project: Entity<Project>,
        pane: Option<&Pane>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn ItemHandle>> {
        let build = self
            .build_project_item_fns_by_type
            .get(&TypeId::of::<T>())?;
        Some(build(item.into_any(), project, pane, window, cx))
    }
}

impl Global for ProjectItemRegistry {}

/// Registers a [ProjectItem] for the app. When opening a file, all the registered
/// items will get a chance to open the file, starting from the project item that
/// was added last.
pub fn register_project_item<I: ProjectItem>(cx: &mut App) {
    cx.default_global::<ProjectItemRegistry>().register::<I>();
}
