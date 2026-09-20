pub mod search;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use alacritty_terminal::event::{Event as AlacTermEvent, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions as _;
use parking_lot::Mutex;
mod content;
mod conversions;
mod dimensions;
mod event;
mod find;
mod process_id;
mod pty;
mod term_ops;
mod types;
use alacritty_terminal::{
    grid::GridIterator,
    sync::FairMutex,
    term::{
        Config, Term,
        cell::{Cell as AlacCell, Flags, Hyperlink as AlacHyperlink},
    },
    tty::{self, Options, Shell},
    vte::ansi::{Color, CursorShape},
};
pub(super) use hyperlinks::{HyperlinkMatch, RegexSearches};

use crate::TerminalBounds;
use crate::alacritty::types::ZedListener;
use crate::cell::{Content, IndexedCell};
use crate::cursor::Range;
mod config;
mod hyperlinks;
pub(crate) use crate::alacritty::{
    config::{display_only_term_config, pty_term_config, set_default_cursor_style},
    content::make_content,
    find::find_from_terminal_point,
    pty::{
        apply_config, current_child_signal_mask, display_offset, new_term, open_pty, pty_options,
        resize, spawn_event_loop,
    },
    search::search_matches,
    term_ops::{
        append_text_to_term, clear_saved_screen, content_text, full_content_range,
        last_non_empty_lines, screen_lines, scroll_display, scroll_to_point, selection_text,
        set_selection, shrink_to_used, toggle_vi_mode, total_lines, update_selection,
        update_selection_to_vi_cursor, update_vi_cursor_for_scroll, used_lines, vi_goto_point,
        vi_motion,
    },
    types::{AlacrittySearch, PtySender},
};

pub(super) type AlacrittyPty = tty::Pty;
pub(super) type AlacrittyTerm = Term<ZedListener>;
pub(super) type AlacrittyTermConfig = Config;
pub(super) type AlacrittyTermLock = FairMutex<AlacrittyTerm>;
pub(super) type AlacrittyCell = AlacCell;
pub(super) type AlacrittyGridIterator<'a> = GridIterator<'a, AlacCell>;
pub(super) type AlacrittyHyperlink = AlacHyperlink;

// Backward-compat re-exports for terminal_view crate (旧 API 名字)
pub type AlacrittyBackend = crate::Terminal;
pub type DisplayCell = crate::cell::Cell;
