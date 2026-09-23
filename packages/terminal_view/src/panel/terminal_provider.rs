use anyhow::Result;
use gpui::{App, Task, Window};
use std::process::ExitStatus;
use task::SpawnInTerminal;
use terminal::Terminal;
use util::{ResultExt, TryFutureExt, defer};

use super::TerminalPanel;

pub(super) struct TerminalProvider(pub(super) gpui::Entity<TerminalPanel>);

impl workspace::TerminalProvider for TerminalProvider {
    fn spawn(
        &self,
        task: SpawnInTerminal,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Result<ExitStatus>>> {
        fn spawn(
            &self,
            task: SpawnInTerminal,
            window: &mut Window,
            cx: &mut App,
        ) -> Task<Option<Result<ExitStatus>>> {
            let terminal_panel = self.0.clone();
            window.spawn(cx, async move |cx| {
                let terminal = terminal_panel
                    .update_in(cx, |terminal_panel, window, cx| {
                        terminal_panel.spawn_task(&task, window, cx)
                    })
                    .ok()?
                    .await;
                match terminal {
                    Ok(terminal) => {
                        let exit_status = terminal
                            .read_with(cx, |terminal, cx| terminal.wait_for_completed_task(cx))
                            .ok()?
                            .await?;
                        Some(Ok(exit_status))
                    }
                    Err(e) => Some(Err(e)),
                }
            })
        }
    }
}
