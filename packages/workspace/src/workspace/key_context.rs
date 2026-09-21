use super::Workspace;
use crate::{dock::Dock, workspace::events::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

impl Workspace {
    pub fn key_context(&self, cx: &App) -> KeyContext {
        let mut context = KeyContext::new_with_defaults();
        context.add("Workspace");
        context.set("keyboard_layout", cx.keyboard_layout().name().to_string());
        if let Some(status) = self
            .debugger_provider
            .as_ref()
            .and_then(|provider| provider.active_thread_state(cx))
        {
            match status {
                ThreadStatus::Running | ThreadStatus::Stepping => {
                    context.add("debugger_running");
                }
                ThreadStatus::Stopped => context.add("debugger_stopped"),
                ThreadStatus::Exited | ThreadStatus::Ended => {}
            }
            // A coarse "there is a live debug session" flag (running, stepping,
            // or stopped at a breakpoint) used to gate debugger controls like
            // step/pause/stop. Distinct from `debugger_running`, which is only
            // true while the program is actually executing.
            if matches!(
                status,
                ThreadStatus::Running | ThreadStatus::Stepping | ThreadStatus::Stopped
            ) {
                context.add("debugger_session");
            }
        }

        if self.left_dock.read(cx).is_open() {
            if let Some(active_panel) = self.left_dock.read(cx).active_panel() {
                context.set("left_dock", active_panel.panel_key());
            }
        }

        if self.right_dock.read(cx).is_open() {
            if let Some(active_panel) = self.right_dock.read(cx).active_panel() {
                context.set("right_dock", active_panel.panel_key());
            }
        }

        if self.bottom_dock.read(cx).is_open() {
            if let Some(active_panel) = self.bottom_dock.read(cx).active_panel() {
                context.set("bottom_dock", active_panel.panel_key());
            }
        }

        context
    }
}
