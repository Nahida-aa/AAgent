use std::ops::RangeInclusive;

use alacritty_terminal::{
    grid::{Dimensions as _, GridIterator, Scroll as AlacScroll},
    index::{Column, Direction as AlacDirection, Line, Point as AlacPoint},
    selection::{
        Selection as AlacSelection, SelectionRange as AlacSelectionRange,
        SelectionType as AlacSelectionType,
    },
    term::{
        RenderableCursor, TermMode,
        cell::{Cell as AlacCell, Flags, Hyperlink as AlacHyperlink},
        search::RegexSearch,
    },
    vi_mode::ViMotion as AlacViMotion,
    vte::ansi::CursorShape as AlacCursorShape,
};
use std::sync::Arc;

use crate::{
    Cell, Cursor, CursorShape, Hyperlink, HyperlinkData, IndexedCell, Modes, Point, Range,
    RenderableCells, Scroll, Selection, SelectionRange, SelectionSide, SelectionType, ViMotion,
    alacritty::types::AlacrittySearch, selection::Search,
};

use super::types::ZedListener; // 视情况

// 自由函数
pub(crate) fn terminal_hyperlink_from_alacritty(hyperlink: AlacHyperlink) -> Hyperlink {
    Hyperlink::from_alacritty(hyperlink)
}
pub(crate) fn terminal_cell_from_alacritty(cell: &AlacCell) -> Cell { Cell { cell: cell.clone() } }
pub(crate) fn terminal_modes_from_alacritty(mode: TermMode) -> Modes {
    let mut terminal_modes = Modes::empty();
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::APP_CURSOR,
        Modes::APP_CURSOR,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::APP_KEYPAD,
        Modes::APP_KEYPAD,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::SHOW_CURSOR,
        Modes::SHOW_CURSOR,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::LINE_WRAP,
        Modes::LINE_WRAP,
    );
    add_terminal_mode(&mut terminal_modes, mode, TermMode::ORIGIN, Modes::ORIGIN);
    add_terminal_mode(&mut terminal_modes, mode, TermMode::INSERT, Modes::INSERT);
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::LINE_FEED_NEW_LINE,
        Modes::LINE_FEED_NEW_LINE,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::FOCUS_IN_OUT,
        Modes::FOCUS_IN_OUT,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::ALTERNATE_SCROLL,
        Modes::ALTERNATE_SCROLL,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::BRACKETED_PASTE,
        Modes::BRACKETED_PASTE,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::SGR_MOUSE,
        Modes::SGR_MOUSE,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::UTF8_MOUSE,
        Modes::UTF8_MOUSE,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::ALT_SCREEN,
        Modes::ALT_SCREEN,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::MOUSE_REPORT_CLICK,
        Modes::MOUSE_REPORT_CLICK,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::MOUSE_DRAG,
        Modes::MOUSE_DRAG,
    );
    add_terminal_mode(
        &mut terminal_modes,
        mode,
        TermMode::MOUSE_MOTION,
        Modes::MOUSE_MOTION,
    );
    add_terminal_mode(&mut terminal_modes, mode, TermMode::VI, Modes::VI);
    terminal_modes
}
pub(crate) fn terminal_cursor_shape_from_alacritty(shape: AlacCursorShape) -> CursorShape {
    match shape {
        AlacCursorShape::Block => CursorShape::Block,
        AlacCursorShape::Underline => CursorShape::Underline,
        AlacCursorShape::Beam => CursorShape::Bar,
        AlacCursorShape::HollowBlock => CursorShape::HollowBlock,
        AlacCursorShape::Hidden => CursorShape::Hidden,
    }
}
pub(crate) fn terminal_point_from_alacritty(point: AlacPoint) -> Point {
    Point {
        line: point.line.0,
        column: point.column.0,
    }
}
pub(crate) fn terminal_selection_range_from_alacritty(range: AlacSelectionRange) -> SelectionRange {
    SelectionRange {
        start: terminal_point_from_alacritty(range.start),
        end: terminal_point_from_alacritty(range.end),
        is_block: range.is_block,
    }
}

fn add_terminal_mode(
    terminal_modes: &mut Modes,
    alacritty_modes: TermMode,
    alacritty_mode: TermMode,
    terminal_mode: Modes,
) {
    if alacritty_modes.contains(alacritty_mode) {
        terminal_modes.insert(terminal_mode);
    }
}
#[cfg(test)]
fn add_alacritty_mode(
    alacritty_modes: &mut TermMode,
    terminal_modes: Modes,
    terminal_mode: Modes,
    alacritty_mode: TermMode,
) {
    if terminal_modes.contains(terminal_mode) {
        alacritty_modes.insert(alacritty_mode);
    }
}

// trait impl（必须能看见类型定义所在模块）
impl Cell {
    #[inline]
    pub fn character(&self) -> char { self.cell.c }

    #[inline]
    pub fn hyperlink(&self) -> Option<Hyperlink> {
        self.cell.hyperlink().map(terminal_hyperlink_from_alacritty)
    }
}
impl<'a> RenderableCells<'a> {
    pub(crate) fn new(cells: GridIterator<'a, AlacCell>) -> Self { Self { cells } }
}
impl Modes {
    #[cfg(test)]
    pub(super) fn to_alacritty(self) -> TermMode {
        let mut mode = TermMode::empty();
        add_alacritty_mode(&mut mode, self, Self::APP_CURSOR, TermMode::APP_CURSOR);
        add_alacritty_mode(&mut mode, self, Self::APP_KEYPAD, TermMode::APP_KEYPAD);
        add_alacritty_mode(&mut mode, self, Self::SHOW_CURSOR, TermMode::SHOW_CURSOR);
        add_alacritty_mode(&mut mode, self, Self::LINE_WRAP, TermMode::LINE_WRAP);
        add_alacritty_mode(&mut mode, self, Self::ORIGIN, TermMode::ORIGIN);
        add_alacritty_mode(&mut mode, self, Self::INSERT, TermMode::INSERT);
        add_alacritty_mode(
            &mut mode,
            self,
            Self::LINE_FEED_NEW_LINE,
            TermMode::LINE_FEED_NEW_LINE,
        );
        add_alacritty_mode(&mut mode, self, Self::FOCUS_IN_OUT, TermMode::FOCUS_IN_OUT);
        add_alacritty_mode(
            &mut mode,
            self,
            Self::ALTERNATE_SCROLL,
            TermMode::ALTERNATE_SCROLL,
        );
        add_alacritty_mode(
            &mut mode,
            self,
            Self::BRACKETED_PASTE,
            TermMode::BRACKETED_PASTE,
        );
        add_alacritty_mode(&mut mode, self, Self::SGR_MOUSE, TermMode::SGR_MOUSE);
        add_alacritty_mode(&mut mode, self, Self::UTF8_MOUSE, TermMode::UTF8_MOUSE);
        add_alacritty_mode(&mut mode, self, Self::ALT_SCREEN, TermMode::ALT_SCREEN);
        add_alacritty_mode(
            &mut mode,
            self,
            Self::MOUSE_REPORT_CLICK,
            TermMode::MOUSE_REPORT_CLICK,
        );
        add_alacritty_mode(&mut mode, self, Self::MOUSE_DRAG, TermMode::MOUSE_DRAG);
        add_alacritty_mode(&mut mode, self, Self::MOUSE_MOTION, TermMode::MOUSE_MOTION);
        add_alacritty_mode(&mut mode, self, Self::VI, TermMode::VI);
        mode
    }
}
impl Cursor {
    pub(crate) fn from_alacritty(cursor: RenderableCursor) -> Self {
        Self {
            shape: terminal_cursor_shape_from_alacritty(cursor.shape),
            point: terminal_point_from_alacritty(cursor.point),
        }
    }
}
impl Point {
    pub(crate) fn to_alacritty(self) -> AlacPoint {
        AlacPoint::new(Line(self.line), Column(self.column))
    }
}
impl Range {
    pub(crate) fn from_alacritty(range: RangeInclusive<AlacPoint>) -> Self {
        Self {
            start: terminal_point_from_alacritty(*range.start()),
            end: terminal_point_from_alacritty(*range.end()),
        }
    }
}
impl Scroll {
    pub(crate) fn to_alacritty(self) -> AlacScroll {
        match self {
            Self::Delta(delta) => AlacScroll::Delta(delta),
            Self::PageUp => AlacScroll::PageUp,
            Self::PageDown => AlacScroll::PageDown,
            Self::Top => AlacScroll::Top,
            Self::Bottom => AlacScroll::Bottom,
        }
    }
}
impl ViMotion {
    pub(crate) fn to_alacritty(self) -> AlacViMotion {
        match self {
            Self::Up => AlacViMotion::Up,
            Self::Down => AlacViMotion::Down,
            Self::Left => AlacViMotion::Left,
            Self::Right => AlacViMotion::Right,
            Self::First => AlacViMotion::First,
            Self::Last => AlacViMotion::Last,
            Self::FirstOccupied => AlacViMotion::FirstOccupied,
            Self::High => AlacViMotion::High,
            Self::Middle => AlacViMotion::Middle,
            Self::Low => AlacViMotion::Low,
            Self::WordLeft => AlacViMotion::WordLeft,
            Self::WordRight => AlacViMotion::WordRight,
            Self::WordRightEnd => AlacViMotion::WordRightEnd,
            Self::Bracket => AlacViMotion::Bracket,
            Self::ParagraphUp => AlacViMotion::ParagraphUp,
            Self::ParagraphDown => AlacViMotion::ParagraphDown,
        }
    }
}
impl Search {
    pub fn new(search: &str) -> Option<Self> {
        Some(Self {
            search: AlacrittySearch {
                search: RegexSearch::new(search).ok()?,
            },
        })
    }

    pub(crate) fn into_alacritty(self) -> RegexSearch { self.search.search }
}
impl SelectionSide {
    pub(crate) fn to_alacritty(self) -> AlacDirection {
        match self {
            Self::Left => AlacDirection::Left,
            Self::Right => AlacDirection::Right,
        }
    }
}
impl SelectionType {
    fn to_alacritty(self) -> AlacSelectionType {
        match self {
            Self::Simple => AlacSelectionType::Simple,
            Self::Semantic => AlacSelectionType::Semantic,
            Self::Lines => AlacSelectionType::Lines,
        }
    }
}
impl Selection {
    pub(crate) fn to_alacritty(&self) -> AlacSelection {
        let mut selection = AlacSelection::new(
            self.ty.to_alacritty(),
            self.start.point.to_alacritty(),
            self.start.side.to_alacritty(),
        );
        if self.start.point != self.end.point || self.start.side != self.end.side {
            selection.update(self.end.point.to_alacritty(), self.end.side.to_alacritty());
        }
        selection
    }
}
impl Hyperlink {
    pub fn uri(&self) -> &str {
        match &self.data {
            HyperlinkData::Alacritty(hyperlink) => hyperlink.uri(),
            HyperlinkData::Owned { uri, .. } => uri,
        }
    }

    fn from_alacritty(hyperlink: AlacHyperlink) -> Self {
        Self {
            data: HyperlinkData::Alacritty(hyperlink),
        }
    }
}
impl From<Hyperlink> for AlacHyperlink {
    fn from(hyperlink: Hyperlink) -> Self {
        match hyperlink.data {
            HyperlinkData::Alacritty(hyperlink) => hyperlink,
            HyperlinkData::Owned { id, uri } => Self::new(id.as_deref(), uri.to_string()),
        }
    }
}
