use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::task::{TaskStatus, task_summary};

use crate::bounds::normalize_terminal_bounds;
use crate::cell::{GridLinesChange, RenderableCells};
use crate::mappings::keys::to_esc_str;
use crate::pty_info::ProcessIdGetter;
use crate::subprocess::convert_lf_to_crlf;
use crate::task::TaskState;
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
    terminal_settings::{AlternateScroll, CursorShape as SettingsCursorShape, TerminalSettings},
};
use gpui::{
    App, Bounds, ClipboardItem, Context, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point as GpuiPoint, ScrollWheelEvent, Task, TouchPhase,
    Window,
};

use std::{
    borrow::Cow,
    cmp::{self, min},
};
use util::shell::ShellKind;

use super::{PtyResources, Terminal, TerminalType}; // task_summary 在 task.rs
use std::process::ExitStatus;

use log::trace;

use crate::alacritty::{
    AlacrittyTerm, append_text_to_term, apply_config, clear_saved_screen, content_text,
    display_offset, find_from_terminal_point, last_non_empty_lines, make_content, resize,
    screen_lines, scroll_display, scroll_to_point, selection_text, set_default_cursor_style,
    set_selection as set_term_selection, toggle_vi_mode as toggle_term_vi_mode, total_lines,
    update_selection as update_term_selection, update_selection_to_vi_cursor,
    update_vi_cursor_for_scroll, used_lines, vi_goto_point, vi_motion,
};
use crate::mappings::colors::to_vte_rgb;
use aa_gpui_kit_theme::ActiveTheme as _;

use super::cell::Content;
use super::colors::get_color_at_index;

impl Terminal {
    pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let term = self.term.clone();
        let mut terminal = term.lock_unfair();
        //Note that the ordering of events matters for event processing
        while let Some(e) = self.events.pop_front() {
            self.process_terminal_event(&e, &mut terminal, window, cx)
        }

        self.last_content = make_content(&terminal, &self.last_content);
        if self.last_content.grid_lines_change == GridLinesChange::Changed {
            debug_assert!(self.last_content.last_hovered_word.is_none());
            self.refresh_hovered_word(window, cx);

            // Because refresh_hovered_word() may result
            // in new events, but will not trigger a repaint
            if !self.events.is_empty() {
                cx.emit(Event::Wakeup);
            }
        }
    }

    pub fn with_renderable_cells<R>(&self, f: impl for<'a> FnOnce(RenderableCells<'a>) -> R) -> R {
        let term = self.term.lock_unfair();
        let content = term.renderable_content();
        f(RenderableCells::new(content.display_iter))
    }

    pub fn get_content(&self) -> String {
        let term = self.term.lock_unfair();
        content_text(&term)
    }

    pub fn last_n_non_empty_lines(&self, n: usize) -> Vec<String> {
        let terminal = self.term.lock_unfair();
        last_non_empty_lines(&terminal, n)
    }

    pub fn write_output(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        // Inject bytes directly into the terminal emulator and refresh the UI.
        // This bypasses the PTY/event loop for display-only terminals.
        let mut previous_byte_was_cr = false;
        let converted = convert_lf_to_crlf(bytes, &mut previous_byte_was_cr);

        let mut term = self.term.lock();
        self.output_processor.advance(&mut *term, &converted);
        drop(term);
        self.detect_init_command_startup_marker();
        cx.emit(Event::Wakeup);
    }

    pub fn total_lines(&self) -> usize { total_lines(&self.term.lock_unfair()) }

    pub fn viewport_lines(&self) -> usize { screen_lines(&self.term.lock_unfair()) }

    pub fn used_lines(&self) -> usize { used_lines(&self.term.lock_unfair()) }

    pub fn last_content(&self) -> &Content { &self.last_content }

    pub fn set_cursor_shape(&mut self, cursor_shape: SettingsCursorShape) {
        set_default_cursor_style(&mut self.term_config, cursor_shape);
        apply_config(&self.term, &self.term_config);
    }
    pub fn focus_in(&self) {
        if self.last_content.mode.contains(Modes::FOCUS_IN_OUT) {
            self.write_to_pty("\x1b[I".as_bytes());
        }
    }

    pub fn focus_out(&mut self) {
        if self.last_content.mode.contains(Modes::FOCUS_IN_OUT) {
            self.write_to_pty("\x1b[O".as_bytes());
        }
    }
}
