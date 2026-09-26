use super::*;
impl Workspace {
    // pub fn add_folder_to_project
    pub fn add_folder_to_project(
        &mut self,
        _: &AddFolderToProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = self.project.read(cx);
        if project.is_via_collab() {
            self.show_error("You cannot add folders to someone else's project", cx);
            return;
        }
        let paths = self.prompt_for_open_path(
            PathPromptOptions {
                files: false,
                directories: true,
                multiple: true,
                prompt: None,
            },
            DirectoryLister::Project(self.project.clone()),
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if let Some(paths) = paths.await.log_err().flatten() {
                let results = this
                    .update_in(cx, |this, window, cx| {
                        this.open_paths(
                            paths,
                            OpenOptions {
                                visible: Some(OpenVisible::All),
                                ..Default::default()
                            },
                            None,
                            window,
                            cx,
                        )
                    })?
                    .await;
                for result in results.into_iter().flatten() {
                    result.log_err();
                }
            }
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    // pub fn project_path_for_path       // 静态方法
    pub fn project_path_for_path(
        project: Entity<Project>,
        abs_path: &Path,
        visible: bool,
        cx: &mut App,
    ) -> Task<Result<(Entity<Worktree>, ProjectPath)>> {
        let entry = project.update(cx, |project, cx| {
            project.find_or_create_worktree(abs_path, visible, cx)
        });
        cx.spawn(async move |cx| {
            let (worktree, path) = entry.await?;
            let worktree_id = worktree.read_with(cx, |t, _| t.id());
            Ok((worktree, ProjectPath { worktree_id, path }))
        })
    }
}
