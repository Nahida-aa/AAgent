use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::task::{TaskStatus, task_summary};

use crate::bounds::normalize_terminal_bounds;
use crate::builder::TerminalBuilder;
use crate::cell::{GridLinesChange, RenderableCells};
use crate::mappings::keys::to_esc_str;
use crate::mode::TerminalMode;
use crate::pty_info::ProcessIdGetter;
use crate::selection::Search;
use crate::task::TaskState;
use crate::terminal_settings::{AlternateScroll, CursorShape as SettingsCursorShape};
use crate::{CwdHistoryEntry, HyperlinkMatch};
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
#[cfg(not(windows))]
use anyhow::Context as _;
use anyhow::{Result, bail};
use gpui::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point as GpuiPoint, ScrollWheelEvent,
    Task, TouchPhase, Window,
};

use std::{
    borrow::Cow,
    cmp::{self, min},
};
use util::{ShellKind, truncate_and_trailoff};

use super::{PtyResources, Terminal, TerminalType}; // task_summary 在 task.rs
use std::process::ExitStatus;

use log::trace;

use crate::alacritty::{
    AlacrittyTerm, append_text_to_term, apply_config, clear_saved_screen, content_text,
    display_offset, find_from_terminal_point, last_non_empty_lines, make_content, resize,
    screen_lines, scroll_display, scroll_to_point, search_matches, selection_text,
    set_default_cursor_style, set_selection as set_term_selection,
    toggle_vi_mode as toggle_term_vi_mode, total_lines, update_selection as update_term_selection,
    update_selection_to_vi_cursor, update_vi_cursor_for_scroll, used_lines, vi_goto_point,
    vi_motion,
};
use crate::mappings::colors::to_vte_rgb;
use aa_gpui_kit_theme::ActiveTheme as _;

use super::cell::Content;
use super::colors::get_color_at_index;

impl Terminal {
    pub fn find_matches(&self, searcher: Search, cx: &Context<Self>) -> Task<Vec<Range>> {
        let term = self.term.clone();
        cx.background_spawn(async move {
            let term = term.lock();
            search_matches(&term, searcher)
        })
    }
    pub fn title(&self, truncate: bool) -> String {
        const MAX_CHARS: usize = 25;
        match &self.task {
            Some(task_state) => {
                if truncate {
                    truncate_and_trailoff(&task_state.spawned_task.label, MAX_CHARS)
                } else {
                    task_state.spawned_task.full_label.clone()
                }
            }
            None => self
                .title_override
                .as_ref()
                .map(|title_override| title_override.to_string())
                .unwrap_or_else(|| match &self.terminal_type {
                    TerminalType::Pty { info, .. } => info
                        .current
                        .read()
                        .as_ref()
                        .map(|fpi| {
                            let process_file = fpi
                                .cwd
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default();

                            let argv = fpi.argv.as_slice();
                            let process_name = format!(
                                "{}{}",
                                fpi.name,
                                if !argv.is_empty() {
                                    format!(" {}", (argv[1..]).join(" "))
                                } else {
                                    "".to_string()
                                }
                            );
                            let (process_file, process_name) = if truncate {
                                (
                                    truncate_and_trailoff(&process_file, MAX_CHARS),
                                    truncate_and_trailoff(&process_name, MAX_CHARS),
                                )
                            } else {
                                (process_file, process_name)
                            };
                            format!("{process_file} — {process_name}")
                        })
                        .unwrap_or_else(|| "Terminal".to_string()),
                    TerminalType::DisplayOnly => "Terminal".to_string(),
                }),
        }
    }

    pub fn clone_builder(&self, cx: &App, cwd: Option<PathBuf>) -> Task<Result<TerminalBuilder>> {
        let working_directory = self.working_directory().or_else(|| cwd);
        TerminalBuilder::new(
            working_directory,
            TerminalMode::interactive(),
            self.template.shell.clone(),
            self.template.env.clone(),
            self.template.cursor_shape,
            self.template.alternate_scroll,
            self.template.max_scroll_history_lines,
            self.template.path_hyperlink_regexes.clone(),
            self.template.path_hyperlink_timeout,
            self.is_remote_terminal,
            self.template.window_id,
            cx,
            self.activation_script.clone(),
            self.path_style,
        )
    }
}
