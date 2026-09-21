use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    pub(super) fn render_notifications(
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
