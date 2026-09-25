use gpui::{App, ClipboardItem, Entity, WeakEntity, Window};
use project::{Project, ProjectPath};
use util::ResultExt as _;

use crate::Toast;
use crate::Workspace;
use crate::notifications::NotificationId;

/// Opens a permalink for the selected file on its Git hosting provider.
pub fn open_file_permalink(
    project: Entity<Project>,
    project_path: ProjectPath,
    workspace: WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) {
    handle_file_permalink(project, project_path, workspace, false, window, cx);
}

/// Copies a permalink for the selected file on its Git hosting provider.
pub fn copy_file_permalink(
    project: Entity<Project>,
    project_path: ProjectPath,
    workspace: WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) {
    handle_file_permalink(project, project_path, workspace, true, window, cx);
}

fn handle_file_permalink(
    project: Entity<Project>,
    project_path: ProjectPath,
    workspace: WeakEntity<Workspace>,
    copy: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let permalink_task = project.update(cx, |project, cx| {
        project.get_file_permalink(&project_path, cx)
    });

    window
        .spawn(cx, async move |cx| match permalink_task.await {
            Ok(permalink) => {
                cx.update(|_, cx| {
                    if copy {
                        cx.write_to_clipboard(ClipboardItem::new_string(permalink.to_string()));
                    } else {
                        cx.open_url(permalink.as_ref());
                    }
                })
                .ok();
            }
            Err(err) => {
                let action = if copy {
                    "copy file permalink"
                } else {
                    "open file permalink"
                };
                let message = format!("Failed to {action}: {err}");
                anyhow::Result::<()>::Err(err).log_err();

                workspace
                    .update(cx, |workspace, cx| {
                        struct FilePermalinkAction;
                        workspace.show_toast(
                            Toast::new(NotificationId::unique::<FilePermalinkAction>(), message),
                            cx,
                        );
                    })
                    .ok();
            }
        })
        .detach();
}
