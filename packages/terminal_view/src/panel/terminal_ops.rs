use std::path::PathBuf;

use anyhow::{Result, anyhow};
use futures::{channel::oneshot, future::join_all};
use gpui::{App, AsyncWindowContext, Context, Entity, Task, TaskExt, WeakEntity, Window};
use itertools::Itertools;
use project::Project;
use task::{RevealStrategy, RevealTarget, Shell, ShellBuilder, SpawnInTerminal};
use terminal::Terminal;
use util::{ResultExt, defer};
use workspace::{ItemId, Pane, Workspace};

use crate::TerminalView;

use super::TerminalPanel;
use super::helpers::is_enabled_in_workspace;

impl TerminalPanel {
    pub fn open_terminal(
        workspace: &mut Workspace,
        action: &workspace::OpenTerminal,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let Some(terminal_panel) = workspace.panel::<Self>(cx) else {
            return;
        };

        terminal_panel
            .update(cx, |panel, cx| {
                panel.add_terminal_shell(
                    action.local,
                    Some(action.working_directory.clone()),
                    RevealStrategy::Always,
                    window,
                    cx,
                )
            })
            .detach_and_log_err(cx);
    }

    pub fn spawn_task(
        &mut self,
        task: &SpawnInTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        let Some(workspace) = self.workspace.upgrade() else {
            return Task::ready(Err(anyhow!("failed to read workspace")));
        };

        let project = workspace.read(cx).project().read(cx);

        if project.is_via_collab() {
            return Task::ready(Err(anyhow!("cannot spawn tasks as a guest")));
        }

        let remote_client = project.remote_client();
        let is_windows = project.path_style(cx).is_windows();
        let remote_shell = remote_client
            .as_ref()
            .and_then(|remote_client| remote_client.read(cx).shell());

        let shell = if let Some(remote_shell) = remote_shell
            && task.shell == Shell::System
        {
            Shell::Program(remote_shell)
        } else {
            task.shell.clone()
        };

        let task = prepare_task_for_spawn(task, &shell, is_windows);

        if task.allow_concurrent_runs && task.use_new_terminal {
            return self.spawn_in_new_terminal(task, window, cx);
        }

        let mut terminals_for_task = self.terminals_for_task(&task.full_label, cx);
        let Some(existing) = terminals_for_task.pop() else {
            return self.spawn_in_new_terminal(task, window, cx);
        };

        let (existing_item_index, task_pane, existing_terminal) = existing;
        if task.allow_concurrent_runs {
            return self.replace_terminal(
                task,
                task_pane,
                existing_item_index,
                existing_terminal,
                window,
                cx,
            );
        }

        let (tx, rx) = oneshot::channel();

        self.deferred_tasks.insert(
            task.id.clone(),
            cx.spawn_in(window, async move |terminal_panel, cx| {
                wait_for_terminals_tasks(terminals_for_task, cx).await;
                let task = terminal_panel.update_in(cx, |terminal_panel, window, cx| {
                    if task.use_new_terminal {
                        terminal_panel.spawn_in_new_terminal(task, window, cx)
                    } else {
                        terminal_panel.replace_terminal(
                            task,
                            task_pane,
                            existing_item_index,
                            existing_terminal,
                            window,
                            cx,
                        )
                    }
                });
                if let Ok(task) = task {
                    tx.send(task.await).ok();
                }
            }),
        );

        cx.spawn(async move |_, _| rx.await?)
    }

    pub(super) fn spawn_in_new_terminal(
        &mut self,
        spawn_task: SpawnInTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        let reveal = spawn_task.reveal;
        let reveal_target = spawn_task.reveal_target;
        match reveal_target {
            RevealTarget::Center => self
                .workspace
                .update(cx, |workspace, cx| {
                    Self::add_center_terminal(workspace, window, cx, |project, cx| {
                        project.create_terminal_task(spawn_task, cx)
                    })
                })
                .unwrap_or_else(|e| Task::ready(Err(e))),
            RevealTarget::Dock => self.add_terminal_task(spawn_task, reveal, window, cx),
        }
    }

    pub(super) fn new_terminal(
        workspace: &mut Workspace,
        action: &workspace::NewTerminal,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let center_pane = workspace.active_pane();
        let center_pane_has_focus = center_pane.focus_handle(cx).contains_focused(window, cx);
        let active_center_item_is_terminal = center_pane
            .read(cx)
            .active_item()
            .is_some_and(|item| item.downcast::<TerminalView>().is_some());

        if center_pane_has_focus && active_center_item_is_terminal {
            let working_directory = default_working_directory(workspace, cx);
            let local = action.local;
            Self::add_center_terminal(workspace, window, cx, move |project, cx| {
                if local {
                    project.create_local_terminal(cx)
                } else {
                    project.create_terminal_shell(working_directory, cx)
                }
            })
            .detach_and_log_err(cx);
            return;
        }

        let Some(terminal_panel) = workspace.panel::<Self>(cx) else {
            return;
        };

        terminal_panel
            .update(cx, |this, cx| {
                this.add_terminal_shell(
                    action.local,
                    default_working_directory(workspace, cx),
                    RevealStrategy::Always,
                    window,
                    cx,
                )
            })
            .detach_and_log_err(cx);
    }

    pub(super) fn terminals_for_task(
        &self,
        label: &str,
        cx: &mut App,
    ) -> Vec<(usize, Entity<Pane>, Entity<TerminalView>)> {
        let Some(workspace) = self.workspace.upgrade() else {
            return Vec::new();
        };

        let pane_terminal_views = |pane: Entity<Pane>| {
            pane.read(cx)
                .items()
                .enumerate()
                .filter_map(|(index, item)| Some((index, item.act_as::<TerminalView>(cx)?)))
                .filter_map(|(index, terminal_view)| {
                    let task_state = terminal_view.read(cx).terminal().read(cx).task()?;
                    if &task_state.spawned_task.full_label == label {
                        Some((index, terminal_view))
                    } else {
                        None
                    }
                })
                .map(move |(index, terminal_view)| (index, pane.clone(), terminal_view))
        };

        self.center
            .panes()
            .into_iter()
            .cloned()
            .flat_map(pane_terminal_views)
            .chain(
                workspace
                    .read(cx)
                    .panes()
                    .iter()
                    .cloned()
                    .flat_map(pane_terminal_views),
            )
            .sorted_by_key(|(_, _, terminal_view)| terminal_view.entity_id())
            .collect()
    }

    pub(super) fn activate_terminal_view(
        &self,
        pane: &Entity<Pane>,
        item_index: usize,
        focus: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        pane.update(cx, |pane, cx| {
            pane.activate_item(item_index, true, focus, window, cx)
        })
    }

    pub fn add_center_terminal(
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Workspace>,
        create_terminal: impl FnOnce(
            &mut Project,
            &mut Context<Project>,
        ) -> Task<Result<Entity<Terminal>>>
        + 'static,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        if !is_enabled_in_workspace(workspace, cx) {
            return Task::ready(Err(anyhow!(
                "terminal not yet supported for remote projects"
            )));
        }
        let project = workspace.project().downgrade();
        cx.spawn_in(window, async move |workspace, cx| {
            let terminal = project.update(cx, create_terminal)?.await?;

            workspace.update_in(cx, |workspace, window, cx| {
                let terminal_view = cx.new(|cx| {
                    TerminalView::new(
                        terminal.clone(),
                        workspace.weak_handle(),
                        workspace.database_id(),
                        workspace.project().downgrade(),
                        window,
                        cx,
                    )
                });
                // Don't steal focus from an open modal (e.g. the command palette):
                // a background terminal can finish starting up after the user has
                // moved on, and focusing it would dismiss whatever they opened.
                let focus_item = !workspace.has_active_modal(window, cx);
                workspace.add_item_to_active_pane(
                    Box::new(terminal_view),
                    None,
                    focus_item,
                    window,
                    cx,
                );
            })?;
            Ok(terminal.downgrade())
        })
    }

    pub fn add_terminal_task(
        &mut self,
        task: SpawnInTerminal,
        reveal_strategy: RevealStrategy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        let workspace = self.workspace.clone();
        self.spawn_pending_terminal(window, cx, async move |terminal_panel, cx| {
            if workspace.update(cx, |workspace, cx| !is_enabled_in_workspace(workspace, cx))? {
                anyhow::bail!("terminal not yet supported for remote projects");
            }
            let project = workspace.read_with(cx, |workspace, _| workspace.project().clone())?;
            let terminal = project
                .update(cx, |project, cx| project.create_terminal_task(task, cx))
                .await?;
            let pane = terminal_panel
                .read_with(cx, |terminal_panel, _| terminal_panel.active_pane.clone())?;
            workspace.update_in(cx, |workspace, window, cx| {
                let terminal_view = Box::new(cx.new(|cx| {
                    TerminalView::new(
                        terminal.clone(),
                        workspace.weak_handle(),
                        workspace.database_id(),
                        workspace.project().downgrade(),
                        window,
                        cx,
                    )
                }));

                let take_focus = reveal_strategy == RevealStrategy::Always
                    && !workspace.has_active_modal(window, cx);
                match reveal_strategy {
                    RevealStrategy::Always if take_focus => {
                        workspace.focus_panel::<Self>(window, cx);
                    }
                    RevealStrategy::Always | RevealStrategy::NoFocus => {
                        workspace.open_panel::<Self>(window, cx);
                    }
                    RevealStrategy::Never => {}
                }

                pane.update(cx, |pane, cx| {
                    pane.add_item(terminal_view, true, take_focus, None, window, cx);
                });

                terminal.downgrade()
            })
        })
    }

    pub(super) fn add_terminal_shell(
        &mut self,
        force_local: bool,
        cwd: Option<PathBuf>,
        reveal_strategy: RevealStrategy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        let workspace = self.workspace.clone();
        self.spawn_pending_terminal(window, cx, async move |terminal_panel, cx| {
            if workspace.update(cx, |workspace, cx| !is_enabled_in_workspace(workspace, cx))? {
                anyhow::bail!("terminal not yet supported for collaborative projects");
            }
            let project = workspace.read_with(cx, |workspace, _| workspace.project().clone())?;
            let terminal = if force_local {
                project
                    .update(cx, |project, cx| project.create_local_terminal(cx))
                    .await
            } else {
                project
                    .update(cx, |project, cx| project.create_terminal_shell(cwd, cx))
                    .await
            };

            let pane = terminal_panel
                .read_with(cx, |terminal_panel, _| terminal_panel.active_pane.clone())?;
            match terminal {
                Ok(terminal) => workspace.update_in(cx, |workspace, window, cx| {
                    let terminal_view = Box::new(cx.new(|cx| {
                        TerminalView::new(
                            terminal.clone(),
                            workspace.weak_handle(),
                            workspace.database_id(),
                            workspace.project().downgrade(),
                            window,
                            cx,
                        )
                    }));

                    let take_focus = reveal_strategy == RevealStrategy::Always
                        && !workspace.has_active_modal(window, cx);
                    match reveal_strategy {
                        RevealStrategy::Always if take_focus => {
                            workspace.focus_panel::<Self>(window, cx);
                        }
                        RevealStrategy::Always | RevealStrategy::NoFocus => {
                            workspace.open_panel::<Self>(window, cx);
                        }
                        RevealStrategy::Never => {}
                    }

                    pane.update(cx, |pane, cx| {
                        pane.add_item(terminal_view, true, take_focus, None, window, cx);
                    });

                    terminal.downgrade()
                }),
                Err(error) => {
                    pane.update_in(cx, |pane, window, cx| {
                        let focus = pane.has_focus(window, cx);
                        let failed_to_spawn = cx.new(|cx| FailedToSpawnTerminal {
                            error: error.to_string(),
                            focus_handle: cx.focus_handle(),
                        });
                        pane.add_item(Box::new(failed_to_spawn), true, focus, None, window, cx);
                    })?;
                    Err(error)
                }
            }
        })
    }

    pub(super) fn spawn_pending_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        create_terminal: impl AsyncFnOnce(
            WeakEntity<Self>,
            &mut AsyncWindowContext,
        ) -> Result<WeakEntity<Terminal>>
        + 'static,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        self.pending_terminals_to_add += 1;
        cx.notify();
        let decrement_when_cancelled = defer({
            let terminal_panel = cx.weak_entity();
            let cx = cx.to_async();
            move || {
                cx.spawn(async move |cx| {
                    terminal_panel
                        .update(cx, |terminal_panel, cx| {
                            terminal_panel.finish_pending_terminal(cx)
                        })
                        .ok();
                })
                .detach();
            }
        });
        cx.spawn_in(window, async move |terminal_panel, cx| {
            let result = create_terminal(terminal_panel.clone(), cx).await;
            decrement_when_cancelled.abort();
            terminal_panel
                .update(cx, |terminal_panel, cx| {
                    terminal_panel.finish_pending_terminal(cx);
                    if result.is_ok() {
                        terminal_panel.serialize(cx);
                    }
                })
                .ok();
            result
        })
    }

    pub(super) fn finish_pending_terminal(&mut self, cx: &mut Context<Self>) {
        self.pending_terminals_to_add = self.pending_terminals_to_add.saturating_sub(1);
        cx.notify();
    }

    pub(super) fn replace_terminal(
        &self,
        spawn_task: SpawnInTerminal,
        task_pane: Entity<Pane>,
        terminal_item_index: usize,
        terminal_to_replace: Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<WeakEntity<Terminal>>> {
        let reveal = spawn_task.reveal;
        let task_workspace = self.workspace.clone();
        cx.spawn_in(window, async move |terminal_panel, cx| {
            let project = terminal_panel.update(cx, |this, cx| {
                this.workspace
                    .update(cx, |workspace, _| workspace.project().clone())
            })??;
            let new_terminal = project
                .update(cx, |project, cx| {
                    project.create_terminal_task(spawn_task, cx)
                })
                .await?;
            terminal_to_replace.update_in(cx, |terminal_to_replace, window, cx| {
                terminal_to_replace.set_terminal(new_terminal.clone(), window, cx);
            })?;

            let reveal_target = terminal_panel.update(cx, |panel, _| {
                if panel.center.panes().iter().any(|p| **p == task_pane) {
                    RevealTarget::Dock
                } else {
                    RevealTarget::Center
                }
            })?;

            match reveal {
                RevealStrategy::Always => match reveal_target {
                    RevealTarget::Center => {
                        task_workspace.update_in(cx, |workspace, window, cx| {
                            let did_activate = workspace.activate_item(
                                &terminal_to_replace,
                                true,
                                true,
                                window,
                                cx,
                            );

                            anyhow::ensure!(did_activate, "Failed to retrieve terminal pane");

                            anyhow::Ok(())
                        })??;
                    }
                    RevealTarget::Dock => {
                        terminal_panel.update_in(cx, |terminal_panel, window, cx| {
                            terminal_panel.activate_terminal_view(
                                &task_pane,
                                terminal_item_index,
                                true,
                                window,
                                cx,
                            )
                        })?;

                        cx.spawn(async move |cx| {
                            task_workspace
                                .update_in(cx, |workspace, window, cx| {
                                    workspace.focus_panel::<Self>(window, cx)
                                })
                                .ok()
                        })
                        .detach();
                    }
                },
                RevealStrategy::NoFocus => match reveal_target {
                    RevealTarget::Center => {
                        task_workspace.update_in(cx, |workspace, window, cx| {
                            workspace.active_pane().focus_handle(cx).focus(window, cx);
                        })?;
                    }
                    RevealTarget::Dock => {
                        terminal_panel.update_in(cx, |terminal_panel, window, cx| {
                            terminal_panel.activate_terminal_view(
                                &task_pane,
                                terminal_item_index,
                                false,
                                window,
                                cx,
                            )
                        })?;

                        cx.spawn(async move |cx| {
                            task_workspace
                                .update_in(cx, |workspace, window, cx| {
                                    workspace.open_panel::<Self>(window, cx)
                                })
                                .ok()
                        })
                        .detach();
                    }
                },
                RevealStrategy::Never => {}
            }

            Ok(new_terminal.downgrade())
        })
    }
}
