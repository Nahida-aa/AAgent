#[cfg(target_os = "windows")]
use std::num::NonZeroU32;
#[cfg(unix)]
use std::os::fd::AsRawFd;

use futures::channel::mpsc::UnboundedSender;
use std::{borrow::Cow, sync::Arc};

use crate::{PtyEvent, TerminalBounds, events::TerminalBackendEvent};

use std::{io, path::PathBuf};

use crate::terminal_settings::{AlternateScroll, CursorShape as SettingsCursorShape};

use alacritty_terminal::{
    event::{Event as AlacTermEvent, EventListener, Notify, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::{Dimensions, Grid, GridIterator, Row, Scroll as AlacScroll},
    index::{Boundary, Column, Direction as AlacDirection, Line, Point as AlacPoint},
    selection::{
        Selection as AlacSelection, SelectionRange as AlacSelectionRange,
        SelectionType as AlacSelectionType,
    },
    sync::FairMutex,
    term::{
        Config, Osc52, RenderableCursor, SEMANTIC_ESCAPE_CHARS, Term, TermMode,
        cell::{Cell as AlacCell, Flags, Hyperlink as AlacHyperlink},
        search::{Match, RegexIter, RegexSearch},
    },
    tty,
    vi_mode::{ViModeCursor, ViMotion as AlacViMotion},
    vte::ansi::{
        ClearMode, CursorShape as AlacCursorShape, CursorStyle as AlacCursorStyle,
        NamedPrivateMode, PrivateMode,
    },
};
use anyhow::{Context as _, Result};

use super::config::pty_term_config;
use super::types::{PtySender, ZedListener};
use super::{AlacrittyPty, AlacrittyTerm, AlacrittyTermConfig, AlacrittyTermLock};

pub(crate) fn window_size_from_terminal_bounds(
    bounds: TerminalBounds,
) -> alacritty_terminal::event::WindowSize {
    WindowSize {
        num_lines: bounds.num_lines() as u16,
        num_cols: bounds.num_columns() as u16,
        cell_width: f32::from(bounds.cell_width()) as u16,
        cell_height: f32::from(bounds.line_height()) as u16,
    }
}
pub(crate) fn apply_config(term: &AlacrittyTermLock, config: &AlacrittyTermConfig) {
    term.lock().set_options(config.clone());
}
#[cfg(not(windows))]
pub(crate) fn current_child_signal_mask() -> io::Result<tty::SignalMask> {
    tty::SignalMask::current()
}

pub(crate) fn pty_options(
    shell: Option<(String, Vec<String>)>,
    working_directory: Option<PathBuf>,
    env: impl IntoIterator<Item = (String, String)>,
    #[cfg(not(windows))] child_signal_mask: Option<tty::SignalMask>,
    #[cfg(windows)] escape_args: bool,
) -> tty::Options {
    tty::Options {
        shell: shell.map(|(program, args)| tty::Shell::new(program, args)),
        working_directory,
        drain_on_exit: true,
        env: env.into_iter().collect(),
        #[cfg(not(windows))]
        child_signal_mask,
        #[cfg(windows)]
        escape_args,
    }
}

pub(crate) fn open_pty(
    options: &tty::Options,
    bounds: TerminalBounds,
    window_id: u64,
) -> io::Result<AlacrittyPty> {
    tty::new(options, window_size_from_terminal_bounds(bounds), window_id)
}

pub(crate) fn new_term(
    config: &AlacrittyTermConfig,
    bounds: TerminalBounds,
    events_tx: UnboundedSender<PtyEvent>,
    alternate_scroll: AlternateScroll,
) -> Arc<AlacrittyTermLock> {
    let mut term = Term::new(config.clone(), &bounds, ZedListener(events_tx));

    if let AlternateScroll::Off = alternate_scroll {
        term.unset_private_mode(PrivateMode::Named(NamedPrivateMode::AlternateScroll));
    }

    Arc::new(FairMutex::new(term))
}

pub(crate) fn spawn_event_loop(
    term: Arc<AlacrittyTermLock>,
    events_tx: UnboundedSender<PtyEvent>,
    pty: AlacrittyPty,
    drain_on_exit: bool,
) -> Result<PtySender> {
    let event_loop = EventLoop::new(term, ZedListener(events_tx), pty, drain_on_exit, false)
        .context("failed to create event loop")?;
    let pty_tx = event_loop.channel();
    let _io_thread = event_loop.spawn();

    Ok(PtySender {
        notifier: Notifier(pty_tx),
    })
}

pub(crate) fn resize(term: &mut AlacrittyTerm, bounds: TerminalBounds) { term.resize(bounds); }

pub(crate) fn display_offset(term: &AlacrittyTerm) -> usize { term.grid().display_offset() }
