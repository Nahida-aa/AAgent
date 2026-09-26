use super::Workspace;
use super::*;
use crate::{dock::Dock, workspace::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    //
    pub(crate) fn render_notifications(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Div> {
        if self.notifications.is_empty() {
            None
        } else {
            Some(
                div()
                    .absolute()
                    .right_3()
                    .bottom_3()
                    .w_112()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .gap_2()
                    .children(
                        self.notifications
                            .iter()
                            .map(|(_, notification)| notification.clone().into_any_element()),
                    ),
            )
        }
    }
}

pub(crate) fn notify_if_database_failed(window: WindowHandle<MultiWorkspace>, cx: &mut AsyncApp) {
    window
        .update(cx, |multi_workspace, _, cx| {
            let workspace = multi_workspace.workspace().clone();
            workspace.update(cx, |workspace, cx| {
                if (*db::ALL_FILE_DB_FAILED).load(std::sync::atomic::Ordering::Acquire) {
                    struct DatabaseFailedNotification;

                    workspace.show_notification(
                        NotificationId::unique::<DatabaseFailedNotification>(),
                        cx,
                        |cx| {
                            cx.new(|cx| {
                                MessageNotification::new("Failed to load the database file.", cx)
                                    .primary_message("File an Issue")
                                    .primary_icon(IconName::Plus)
                                    .primary_on_click(|window, cx| {
                                        window.dispatch_action(Box::new(FileBugReport), cx)
                                    })
                            })
                        },
                    );
                }
            });
        })
        .log_err();
}
