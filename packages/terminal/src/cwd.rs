use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::alacritty::{clear_saved_screen, last_non_empty_lines, make_content};
use crate::mappings::keys::to_esc_str;
use crate::{CwdHistoryEntry, Event, TerminalType};
use crate::{
    TerminalBounds,
    cursor::Point,
    events::InternalEvent,
    mappings::mouse::{grid_point, grid_point_and_side, mouse_button_report, mouse_moved_report},
    modes::Modes,
    selection::{Scroll, Selection, SelectionPhase, SelectionSide, SelectionType, ViMotion},
    terminal_settings::TerminalSettings,
};
use gpui::{
    Bounds, Context, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point as GpuiPoint, ScrollWheelEvent, Task, TouchPhase, Window,
};
use std::{
    borrow::Cow,
    cmp::{self, min},
};
use util::ShellKind;

use super::Terminal;

impl Terminal {
    pub(crate) fn record_cwd_change(&mut self, new_working_directory: PathBuf) {
        if self.is_remote_terminal {
            return;
        }

        let scrollback_position = self.pending_cwd_boundary.take().unwrap_or_else(|| {
            let term = self.term.lock_unfair();
            Self::scrollback_position(term.grid().cursor.point.line.0, term.history_size())
        });
        self.cwd_history.push(CwdHistoryEntry {
            scrollback_position,
            working_directory: new_working_directory,
        });
    }

    pub(crate) fn reset_cwd_history(&mut self) {
        self.pending_cwd_boundary = None;
        self.cwd_history = self
            .working_directory()
            .map(|working_directory| {
                vec![CwdHistoryEntry {
                    scrollback_position: i32::MIN,
                    working_directory,
                }]
            })
            .unwrap_or_default();
    }

    pub(crate) fn cwd_at_line(&self, line: i32, history_size: usize) -> Option<PathBuf> {
        // Once the scrollback cap is reached, evictions move retained lines without changing
        // `history_size`, so stored row offsets no longer identify their original lines.
        if self.is_remote_terminal
            || self.cwd_history.is_empty()
            || history_size >= self.term_config.scrolling_history
        {
            return self.working_directory();
        }
        let scrollback_position = Self::scrollback_position(line, history_size);
        self.cwd_history
            .iter()
            .rev()
            .find(|entry| entry.scrollback_position <= scrollback_position)
            .map(|entry| entry.working_directory.clone())
            .or_else(|| self.working_directory())
    }

    pub(crate) fn scrollback_position(line: i32, history_size: usize) -> i32 {
        let history_size = i32::try_from(history_size).unwrap_or(i32::MAX);
        history_size.saturating_add(line)
    }
    pub fn working_directory(&self) -> Option<PathBuf> {
        if self.is_remote_terminal {
            // We can't yet reliably detect the working directory of a shell on the
            // SSH host. Until we can do that, it doesn't make sense to display
            // the working directory on the client and persist that.
            None
        } else {
            self.client_side_working_directory()
        }
    }

    /// Returns the working directory of the process that's connected to the PTY.
    /// That means it returns the working directory of the local shell or program
    /// that's running inside the terminal.
    ///
    /// This does *not* return the working directory of the shell that runs on the
    /// remote host, in case Zed is connected to a remote host.
    fn client_side_working_directory(&self) -> Option<PathBuf> {
        match &self.terminal_type {
            TerminalType::Pty { info, .. } => info
                .current
                .read()
                .as_ref()
                .map(|process| process.cwd.clone()),
            TerminalType::DisplayOnly => None,
        }
    }
}
