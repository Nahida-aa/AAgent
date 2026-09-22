use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::task::{TaskStatus, task_summary};

use crate::bounds::normalize_terminal_bounds;
use crate::mappings::keys::to_esc_str;
use crate::pty_info::ProcessIdGetter;
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
    pub(crate) fn process_hyperlink(
        &mut self,
        hyperlink: HyperlinkMatch,
        open: bool,
        history_size: usize,
        cx: &mut Context<Self>,
    ) {
        let HyperlinkMatch {
            text: maybe_url_or_path,
            is_url,
            range,
        } = hyperlink;
        let prev_hovered_word = self.last_content.last_hovered_word.take();
        let match_line = range.start().line;
        let working_directory = self.cwd_at_line(match_line, history_size);

        let target = if is_url {
            if let Some(path) = maybe_url_or_path.strip_prefix("file://") {
                let decoded_path = urlencoding::decode(path)
                    .map(|decoded| decoded.into_owned())
                    .unwrap_or(path.to_owned());

                MaybeNavigationTarget::PathLike(PathLikeTarget {
                    maybe_path: decoded_path,
                    working_directory,
                })
            } else {
                MaybeNavigationTarget::Url(maybe_url_or_path.clone())
            }
        } else {
            MaybeNavigationTarget::PathLike(PathLikeTarget {
                maybe_path: maybe_url_or_path.clone(),
                working_directory,
            })
        };

        if open {
            cx.emit(Event::Open(target));
        } else {
            self.update_selected_word(prev_hovered_word, range, maybe_url_or_path, target, cx);
        }
    }

    pub(crate) fn clear_hyperlink(&mut self, cx: &mut Context<Self>) {
        if self.last_content.last_hovered_word.take().is_some() {
            cx.emit(Event::NewNavigationTarget(None));
        }
    }

    pub(crate) fn find_hyperlink_at_point(&mut self, point: Point) -> Option<HyperlinkMatch> {
        let term_lock = self.term.lock();
        find_from_terminal_point(
            &term_lock,
            point,
            &mut self.hyperlink_regex_searches,
            self.path_style,
        )
    }

    fn update_selected_word(
        &mut self,
        prev_word: Option<HoveredWord>,
        word_match: Range,
        word: String,
        navigation_target: MaybeNavigationTarget,
        cx: &mut Context<Self>,
    ) {
        if let Some(prev_word) = prev_word
            && prev_word.word == word
            && prev_word.word_match == word_match
        {
            self.last_content.last_hovered_word = Some(prev_word);
            return;
        }

        self.last_content.last_hovered_word = Some(HoveredWord {
            word,
            word_match,
            id: self.next_link_id(),
        });
        cx.emit(Event::NewNavigationTarget(Some(navigation_target)));
        cx.notify()
    }

    fn next_link_id(&mut self) -> usize {
        let res = self.next_link_id;
        self.next_link_id = self.next_link_id.wrapping_add(1);
        res
    }
}
