use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::Event;
use crate::alacritty::{
    clear_saved_screen, full_content_range, last_non_empty_lines, make_content, shrink_to_used,
};
use crate::bounds::normalize_terminal_bounds;
use crate::cursor::Range;
use crate::mappings::keys::to_esc_str;
use crate::mappings::mouse::{alt_scroll, scroll_report};
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
use util::shell::ShellKind;

use super::Terminal;

impl Terminal {
    pub fn scroll_line_up(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(Scroll::Delta(1)));
    }

    pub fn scroll_up_by(&mut self, lines: usize) {
        self.events
            .push_back(InternalEvent::Scroll(Scroll::Delta(lines as i32)));
    }

    pub fn scroll_line_down(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(Scroll::Delta(-1)));
    }

    pub fn scroll_down_by(&mut self, lines: usize) {
        self.events
            .push_back(InternalEvent::Scroll(Scroll::Delta(-(lines as i32))));
    }

    pub fn scroll_page_up(&mut self) {
        self.events.push_back(InternalEvent::Scroll(Scroll::PageUp));
    }

    pub fn scroll_page_down(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(Scroll::PageDown));
    }

    pub fn scroll_to_top(&mut self) { self.events.push_back(InternalEvent::Scroll(Scroll::Top)); }

    pub fn scroll_to_bottom(&mut self) {
        self.events.push_back(InternalEvent::Scroll(Scroll::Bottom));
    }

    pub fn activate_match(&mut self, index: usize) {
        if let Some(search_match) = self.matches.get(index).cloned() {
            self.set_selection(Some(Selection::simple_range(search_match)));
            if self.vi_mode_enabled {
                self.events
                    .push_back(InternalEvent::MoveViCursorToPoint(search_match.end()));
            } else {
                self.events
                    .push_back(InternalEvent::ScrollToPoint(search_match.start()));
            }
        }
    }

    pub fn select_matches(&mut self, matches: &[Range]) {
        let matches_to_select = self
            .matches
            .iter()
            .filter(|self_match| matches.contains(self_match))
            .cloned()
            .collect::<Vec<_>>();
        for match_to_select in matches_to_select {
            self.set_selection(Some(Selection::simple_range(match_to_select)));
        }
    }

    pub fn select_all(&mut self) {
        let term = self.term.lock();
        let range = full_content_range(&term);
        drop(term);
        self.set_selection(Some(Selection::simple_range(range)));
    }

    fn set_selection(&mut self, selection: Option<Selection>) {
        self.events
            .push_back(InternalEvent::SetSelection(selection));
    }

    pub fn copy(&mut self, keep_selection: Option<bool>) {
        self.events.push_back(InternalEvent::Copy(keep_selection));
    }

    pub fn clear(&mut self) { self.events.push_back(InternalEvent::Clear) }

    pub fn shrink_to_used(&mut self) { shrink_to_used(&mut self.term.lock()); }

    ///Resize the terminal and the PTY.
    pub fn set_size(&mut self, new_bounds: TerminalBounds) {
        let new_bounds = normalize_terminal_bounds(new_bounds);

        let old_bounds = self.last_content.terminal_bounds;
        self.last_content.terminal_bounds = new_bounds;

        // Avoid spamming PTY resizes on pixel-level size changes (e.g. while dragging edges),
        // since those can generate excessive SIGWINCH/reflows and cause visible flicker.
        let requires_resize = old_bounds.num_lines() != new_bounds.num_lines()
            || old_bounds.num_columns() != new_bounds.num_columns()
            || old_bounds.cell_width != new_bounds.cell_width
            || old_bounds.line_height != new_bounds.line_height;

        if !requires_resize {
            return;
        }

        match self.events.back_mut() {
            Some(InternalEvent::Resize(pending_bounds)) => *pending_bounds = new_bounds,
            _ => self.events.push_back(InternalEvent::Resize(new_bounds)),
        }
    }
}
