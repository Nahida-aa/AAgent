use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::task::{TaskStatus, task_summary};
use crate::CwdHistoryEntry;

use crate::bounds::normalize_terminal_bounds;
use crate::cmd::normalize_path_command_name;
use crate::mappings::keys::to_esc_str;
use crate::pty_info::ProcessIdGetter;
use crate::task::TaskState;
use crate::{
    TerminalBounds,
    cursor::{Point, Range},
    events::{
        Event, InternalEvent, MaybeNavigationTarget, PathLikeTarget, PtyEvent, TerminalBackendEvent,
    },
    mappings::mouse::{grid_point, grid_point_and_side, mouse_button_report, mouse_moved_report},
    modes::Modes,
    selection::{
        HoveredWord, Scroll, Selection, SelectionPhase, SelectionSide, SelectionType, ViMotion,
    },
    terminal_settings::TerminalSettings,
};
use gpui::{
    App, Bounds, ClipboardItem, Context, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point as GpuiPoint, ScrollWheelEvent, Task, TouchPhase,
    Window,
}; // 如果 process_hyperlink 在另一个文件

use std::{
    borrow::Cow,
    cmp::{self, min},
};
use util::shell::ShellKind;

use super::{PtyResources, Terminal, TerminalType}; // task_summary 在 task.rs
use std::process::ExitStatus;

use log::trace;

use crate::alacritty::{
    AlacrittyTerm, append_text_to_term, clear_saved_screen, display_offset,
    find_from_terminal_point, last_non_empty_lines, make_content, resize, scroll_display,
    scroll_to_point, selection_text, set_selection as set_term_selection,
    toggle_vi_mode as toggle_term_vi_mode, update_selection as update_term_selection,
    update_selection_to_vi_cursor, update_vi_cursor_for_scroll, vi_goto_point, vi_motion,
};
use crate::mappings::colors::to_vte_rgb;
use aa_gpui_kit_theme::ActiveTheme as _;

use super::cell::Content;
use super::colors::get_color_at_index;

impl Terminal {
    pub fn is_pty(&self) -> bool { matches!(self.terminal_type, TerminalType::Pty { .. }) }

    /// Returns whether this terminal still owns its live PTY sender.
    pub fn has_active_pty_resources(&self) -> bool {
        matches!(
            self.terminal_type,
            TerminalType::Pty {
                resources: PtyResources::Active(_),
                ..
            }
        )
    }

    /// Releases live PTY resources while retaining process metadata and buffered output.
    ///
    /// Calling this method after the resources have already been released is a no-op.
    pub fn release_pty_resources(&mut self) {
        let TerminalType::Pty { resources, info } = &mut self.terminal_type else {
            return;
        };
        let PtyResources::Active(pty_tx) = std::mem::replace(resources, PtyResources::Released)
        else {
            return;
        };
        let info = info.clone();

        pty_tx.shutdown();
        info.terminate_child_process();

        let timer = self.background_executor.timer(Duration::from_millis(100));
        self.background_executor
            .spawn(async move {
                timer.await;
                info.kill_child_process();
            })
            .detach();
    }

    pub fn pid(&self) -> Option<sysinfo::Pid> {
        match &self.terminal_type {
            TerminalType::Pty { info, .. } => info.pid(),
            TerminalType::DisplayOnly => None,
        }
    }

    pub fn pid_getter(&self) -> Option<&ProcessIdGetter> {
        match &self.terminal_type {
            TerminalType::Pty { info, .. } => Some(info.pid_getter()),
            TerminalType::DisplayOnly => None,
        }
    }

    pub fn task(&self) -> Option<&TaskState> { self.task.as_ref() }

    pub fn wait_for_completed_task(&self, cx: &App) -> Task<Option<ExitStatus>> {
        if let Some(task) = self.task() {
            if task.status == TaskStatus::Running {
                let completion_receiver = task.completion_rx.clone();
                return cx.spawn(async move |_| completion_receiver.recv().await.ok().flatten());
            } else if let Ok(status) = task.completion_rx.try_recv() {
                return Task::ready(status);
            }
        }
        Task::ready(None)
    }

    pub fn kill_active_task(&mut self) {
        if let Some(task) = self.task()
            && task.status == TaskStatus::Running
        {
            match &self.terminal_type {
                TerminalType::Pty { info, .. } => {
                    // First kill the foreground process group (the command running in the shell)
                    info.kill_current_process();
                    // Then kill the shell itself so that the terminal exits properly
                    // and wait_for_completed_task can complete
                    info.kill_child_process();
                }
                TerminalType::DisplayOnly => {
                    // Non-PTY task terminals own their subprocess directly.
                    if let Some(subprocess) = &self.subprocess {
                        subprocess.kill();
                    }
                }
            }
        }
    }
    /// Normalizes the command name of the foreground process, if one is known.
    pub fn foreground_process_command_name(&self) -> Option<String> {
        match &self.terminal_type {
            TerminalType::Pty { info, .. } => info
                .current
                .read()
                .as_ref()
                .and_then(|process| foreground_process_command_from_argv(&process.argv)),
            TerminalType::DisplayOnly => None,
        }
    }
}

fn foreground_process_command_from_argv(argv: &[String]) -> Option<String> {
    let command = argv
        .first()
        .and_then(|command| normalize_path_command_name(command));

    if !matches!(
        command.as_deref(),
        Some("node" | "python" | "python3" | "bun" | "deno")
    ) {
        return command;
    }

    argv.iter()
        .skip(1)
        .filter_map(|argument| normalize_script_command_name(argument))
        .next()
        .or(command)
}
fn normalize_script_command_name(argument: &str) -> Option<String> {
    let path = Path::new(argument);
    let file_stem = path
        .file_stem()
        .and_then(|file_stem| file_stem.to_str())
        .and_then(normalize_path_command_name)?;

    if file_stem != "index" {
        return Some(file_stem);
    }

    path.parent()
        .and_then(|parent| parent.parent())
        .and_then(|package_path| package_path.file_name())
        .and_then(|package_name| package_name.to_str())
        .and_then(|package_name| package_name.strip_suffix("-cli").or(Some(package_name)))
        .and_then(normalize_path_command_name)
}
