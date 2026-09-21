use std::time::{Duration, Instant};

use anyhow::Result;
use db::kvp::KeyValueStore;
use gpui::{AsyncWindowContext, Context, Entity, Task, TaskExt, WeakEntity, Window};
use serde_json;
use util::{ResultExt, TryFutureExt};
use workspace::{ItemId, Workspace};

use crate::TerminalView;
use crate::persistence::{
    SerializedItems, SerializedTerminalPanel, deserialize_terminal_panel, serialize_pane_group,
};

use super::TerminalPanel;
use super::actions::TERMINAL_PANEL_KEY;
use super::helpers::default_working_directory;

impl TerminalPanel {
    pub(super) fn serialization_key(workspace: &Workspace) -> Option<String> {
        workspace
            .database_id()
            .map(|id| i64::from(id).to_string())
            .or(workspace.session_id())
            .map(|id| format!("{:?}-{:?}", TERMINAL_PANEL_KEY, id))
    }

    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        let terminal_panel = workspace.update_in(&mut cx, |workspace, window, cx| {
            cx.new(|cx| TerminalPanel::new(workspace, window, cx))
        })?;

        workspace
            .update(&mut cx, |workspace, _| {
                workspace.set_terminal_provider(TerminalProvider(terminal_panel.clone()))
            })
            .ok();

        terminal_panel.update_in(&mut cx, |panel, window, cx| {
            panel.restoring = true;
            panel._restoration = cx.spawn_in(window, {
                let workspace = workspace.clone();
                async move |terminal_panel, cx| {
                    let restored =
                        Self::restore_serialized_state(workspace, terminal_panel.clone(), cx)
                            .await
                            .log_err()
                            .unwrap_or(false);
                    let default_shell_task = terminal_panel
                        .update_in(cx, |terminal_panel, window, cx| {
                            terminal_panel.finish_restoration(restored, window, cx)
                        })
                        .ok()
                        .flatten();
                    if let Some(task) = default_shell_task {
                        task.await.log_err();
                    }
                }
            });
        })?;

        Ok(terminal_panel)
    }

    pub(super) fn finish_restoration(
        &mut self,
        restored: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<WeakEntity<Terminal>>>> {
        self.restoring = false;
        let has_terminals = self
            .center
            .panes()
            .into_iter()
            .any(|pane| pane.read(cx).items_len() > 0);
        if restored || has_terminals {
            self.serialize(cx);
        }
        cx.notify();
        if self.active && self.has_no_terminals(cx) {
            let working_directory = self
                .workspace
                .update(cx, |workspace, cx| default_working_directory(workspace, cx))
                .ok()
                .flatten();
            Some(self.add_terminal_shell(
                false,
                working_directory,
                RevealStrategy::Always,
                window,
                cx,
            ))
        } else {
            None
        }
    }

    pub(super) async fn restore_serialized_state(
        workspace: WeakEntity<Workspace>,
        terminal_panel: WeakEntity<Self>,
        cx: &mut AsyncWindowContext,
    ) -> Result<bool> {
        let mut restored = false;
        if let Some((database_id, serialization_key, kvp)) = workspace
            .read_with(cx, |workspace, cx| {
                workspace
                    .database_id()
                    .zip(TerminalPanel::serialization_key(workspace))
                    .map(|(id, key)| (id, key, KeyValueStore::global(cx)))
            })
            .ok()
            .flatten()
            && let Some(serialized_panel) = cx
                .background_spawn(async move { kvp.read_kvp(&serialization_key) })
                .await
                .log_err()
                .flatten()
                .map(|panel| serde_json::from_str::<SerializedTerminalPanel>(&panel))
                .transpose()
                .log_err()
                .flatten()
        {
            let started_at = std::time::Instant::now();
            let deserialized = workspace
                .update_in(cx, |workspace, window, cx| {
                    deserialize_terminal_panel(
                        workspace.weak_handle(),
                        workspace.project().clone(),
                        database_id,
                        serialized_panel,
                        terminal_panel.clone(),
                        window,
                        cx,
                    )
                })?
                .await;
            if let Some(restored_terminals) = deserialized.log_err() {
                restored = restored_terminals > 0;
                log::debug!(
                    "terminal panel: restored {restored_terminals} serialized terminal(s) in {:?}",
                    started_at.elapsed()
                );
            }
        }

        // Since panels/docks are loaded outside from the workspace, we cleanup here, instead of through the workspace.
        let cleanup = workspace.update_in(cx, |workspace, window, cx| {
            let alive_item_ids = terminal_panel.upgrade().map(|terminal_panel| {
                terminal_panel
                    .read(cx)
                    .center
                    .panes()
                    .into_iter()
                    .flat_map(|pane| pane.read(cx).items())
                    .map(|item| item.item_id().as_u64() as ItemId)
                    .collect::<Vec<_>>()
            });
            alive_item_ids
                .zip(workspace.database_id())
                .map(|(alive_item_ids, workspace_id)| {
                    let cleanup_task =
                        TerminalView::cleanup(workspace_id, alive_item_ids.clone(), window, cx);
                    (cleanup_task, alive_item_ids)
                })
        })?;
        if let Some((cleanup_task, alive_item_ids)) = cleanup {
            cleanup_task.await.log_err();
            terminal_panel
                .update(cx, |terminal_panel, cx| {
                    let terminals_to_reserialize = terminal_panel
                        .center
                        .panes()
                        .into_iter()
                        .flat_map(|pane| {
                            pane.read(cx)
                                .items()
                                .filter(|item| {
                                    !alive_item_ids.contains(&(item.item_id().as_u64() as ItemId))
                                })
                                .filter_map(|item| item.act_as::<TerminalView>(cx))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    for terminal_view in terminals_to_reserialize {
                        terminal_view.update(cx, |terminal_view, cx| {
                            terminal_view.mark_needs_serialize(cx)
                        });
                    }
                })
                .ok();
        }

        let should_focus = workspace
            .update_in(cx, |workspace, window, cx| {
                !workspace.has_active_modal(window, cx)
                    && terminal_panel.upgrade().is_some_and(|terminal_panel| {
                        workspace.active_item(cx).is_none()
                            && workspace
                                .is_dock_at_position_open(terminal_panel.position(window, cx), cx)
                    })
            })
            .unwrap_or(false);
        if should_focus {
            terminal_panel
                .update_in(cx, |panel, window, cx| {
                    panel.active_pane.update(cx, |pane, cx| {
                        pane.focus_active_item(window, cx);
                    });
                })
                .ok();
        }
        Ok(restored)
    }

    pub(super) fn serialize(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        let Some(serialization_key) = self
            .workspace
            .read_with(cx, |workspace, _| {
                TerminalPanel::serialization_key(workspace)
            })
            .ok()
            .flatten()
        else {
            return;
        };
        let kvp = KeyValueStore::global(cx);
        self.pending_serialization = cx.spawn(async move |terminal_panel, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let terminal_panel = terminal_panel.upgrade()?;
            let items = terminal_panel.update(cx, |terminal_panel, cx| {
                SerializedItems::WithSplits(serialize_pane_group(
                    &terminal_panel.center,
                    &terminal_panel.active_pane,
                    cx,
                ))
            });
            cx.background_spawn(
                async move {
                    kvp.write_kvp(
                        serialization_key,
                        serde_json::to_string(&SerializedTerminalPanel {
                            items,
                            active_item_id: None,
                        })?,
                    )
                    .await?;
                    anyhow::Ok(())
                }
                .log_err(),
            )
            .await;
            Some(())
        });
    }
}
