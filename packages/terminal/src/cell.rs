use std::ops::Deref;

use crate::alacritty::{AlacrittyCell, AlacrittyGridIterator};

use super::bounds::TerminalBounds;
use super::cursor::{Cursor, CursorShape, Point, SelectionRange};
use super::modes::Modes;
use super::selection::HoveredWord;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Cell {
    pub(crate) cell: AlacrittyCell,
}
pub struct RenderableCells<'a> {
    pub(crate) cells: AlacrittyGridIterator<'a>,
}
#[derive(Debug, Clone)]
pub struct IndexedCell {
    pub point: Point,
    pub cell: Cell,
}

impl Deref for IndexedCell {
    type Target = Cell;

    #[inline]
    fn deref(&self) -> &Cell { &self.cell }
}

// TODO: Un-pub
#[derive(Clone)]
pub struct Content {
    pub cells: Vec<IndexedCell>,
    pub mode: Modes,
    pub total_lines: usize,
    pub display_offset: usize,
    pub columns: usize,
    pub screen_lines: usize,
    pub selection_text: Option<String>,
    pub selection: Option<SelectionRange>,
    pub cursor: Cursor,
    pub cursor_char: char,
    pub terminal_bounds: TerminalBounds,
    pub last_hovered_word: Option<HoveredWord>,
    pub grid_lines_change: GridLinesChange,
    pub scrolled_to_top: bool,
    pub scrolled_to_bottom: bool,
    pub bottom_row_occupied: bool,
}
impl Default for Content {
    fn default() -> Self {
        Content {
            cells: Default::default(),
            mode: Default::default(),
            total_lines: Default::default(),
            display_offset: Default::default(),
            columns: Default::default(),
            screen_lines: Default::default(),
            selection_text: Default::default(),
            selection: Default::default(),
            cursor: Cursor {
                shape: CursorShape::Block,
                point: Point::new(0, 0),
            },
            cursor_char: Default::default(),
            terminal_bounds: Default::default(),
            last_hovered_word: None,
            grid_lines_change: Default::default(),
            scrolled_to_top: false,
            scrolled_to_bottom: false,
            bottom_row_occupied: false,
        }
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub enum GridLinesChange {
    #[default]
    Unchanged,
    Changed,
}
