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

use crate::alacritty::pty::ZedListener;

pub(crate) type AlacrittyPty = tty::Pty;
pub(crate) type AlacrittyTerm = Term<ZedListener>;
pub(crate) type AlacrittyTermConfig = Config;
pub(crate) type AlacrittyTermLock = FairMutex<AlacrittyTerm>;
pub(crate) type AlacrittyCell = AlacCell;
pub(crate) type AlacrittyGridIterator<'a> = GridIterator<'a, AlacCell>;
pub(crate) type AlacrittyHyperlink = AlacHyperlink;
