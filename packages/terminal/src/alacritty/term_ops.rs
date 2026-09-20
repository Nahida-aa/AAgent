use alacritty_terminal::{
    grid::{Dimensions as _, Grid, Row, Scroll as AlacScroll},
    index::{Column, Direction as AlacDirection, Line, Point as AlacPoint},
    selection::Selection as AlacSelection,
    term::{
        Term, TermMode,
        cell::{Cell as AlacCell, Flags},
        search::{Match, RegexIter, RegexSearch},
    },
    vi_mode::{ViModeCursor, ViMotion as AlacViMotion},
    vte::ansi::{
        ClearMode, CursorShape as AlacCursorShape, CursorStyle as AlacCursorStyle,
        NamedPrivateMode, PrivateMode,
    },
};
use util::paths::PathStyle;
use vte::ansi::Handler;

use crate::{
    Content, GridLinesChange, HoveredWord, Point, Range, Scroll, Selection, SelectionRange,
    SelectionSide, SelectionType, ViMotion, alacritty::AlacrittyTerm,
};

use super::content::*;
use super::conversions::*;
use super::types::{AlacrittySearch, ZedListener};

pub(crate) fn scroll_display(term: &mut Term<ZedListener>, scroll: Scroll) {
    term.scroll_display(scroll.to_alacritty());
}
pub(crate) fn set_selection(term: &mut Term<ZedListener>, selection: Option<&Selection>) {
    term.selection = selection.map(Selection::to_alacritty);
}
pub(crate) fn update_selection(
    term: &mut AlacrittyTerm,
    point: Point,
    side: SelectionSide,
) -> bool {
    let Some(mut selection) = term.selection.take() else {
        return false;
    };
    selection.update(point.to_alacritty(), side.to_alacritty());
    term.selection = Some(selection);
    true
}
pub(crate) fn selection_text(term: &Term<ZedListener>) -> Option<String> {
    term.selection_to_string()
}
pub(crate) fn scroll_to_point(term: &mut AlacrittyTerm, point: Point) {
    term.scroll_to_point(point.to_alacritty());
}
pub(crate) fn vi_goto_point(term: &mut AlacrittyTerm, point: Point) {
    term.vi_goto_point(point.to_alacritty());
}
pub(crate) fn toggle_vi_mode(term: &mut Term<ZedListener>) { term.toggle_vi_mode(); }
pub(crate) fn vi_motion(term: &mut Term<ZedListener>, motion: ViMotion) {
    term.vi_motion(motion.to_alacritty());
}
pub(crate) fn clear_saved_screen(term: &mut Term<ZedListener>) {
    term.clear_screen(ClearMode::Saved);

    let cursor = term.grid().cursor.point;

    term.grid_mut().reset_region(..cursor.line);

    let line = term.grid()[cursor.line][..Column(term.grid().columns())]
        .iter()
        .cloned()
        .enumerate()
        .collect::<Vec<(usize, AlacCell)>>();

    for (index, cell) in line {
        term.grid_mut()[Line(0)][Column(index)] = cell;
    }

    term.grid_mut().cursor.point = AlacPoint::new(Line(0), term.grid_mut().cursor.point.column);
    let new_cursor = term.grid().cursor.point;

    if (new_cursor.line.0 as usize) < term.screen_lines() - 1 {
        term.grid_mut().reset_region((new_cursor.line + 1)..);
    }
}
pub(crate) fn shrink_to_used(term: &mut Term<ZedListener>) { term.grid_mut().truncate(); }
pub(crate) fn used_lines(term: &Term<ZedListener>) -> usize {
    if term.mode().contains(TermMode::ALT_SCREEN) {
        return term.total_lines();
    }
    let grid = term.grid();
    let cursor_line = grid.cursor.point.line.0.max(0) as usize;
    let last_occupied_line = (cursor_line + 1..term.screen_lines())
        .rev()
        .find(|&line| !grid[Line(line as i32)].is_clear())
        .unwrap_or(cursor_line);
    term.history_size() + last_occupied_line + 1
}
pub(crate) fn total_lines(term: &Term<ZedListener>) -> usize { term.total_lines() }
pub(crate) fn screen_lines(term: &Term<ZedListener>) -> usize { term.screen_lines() }
pub(crate) fn full_content_range(term: &Term<ZedListener>) -> Range {
    let start = AlacPoint::new(term.topmost_line(), Column(0));
    let end = AlacPoint::new(term.bottommost_line(), term.last_column());
    Range::from_alacritty(start..=end)
}
pub(crate) fn content_text(term: &Term<ZedListener>) -> String {
    let start = AlacPoint::new(term.topmost_line(), Column(0));
    let end = AlacPoint::new(term.bottommost_line(), term.last_column());
    term.bounds_to_string(start, end)
}
pub(crate) fn last_non_empty_lines(term: &Term<ZedListener>, line_count: usize) -> Vec<String> {
    let grid = term.grid();
    let mut lines = Vec::new();

    let mut current_line = grid.bottommost_line().0;
    let topmost_line = grid.topmost_line().0;

    while current_line >= topmost_line && lines.len() < line_count {
        let (logical_line_start, logical_line) =
            logical_line_for_row(grid, current_line, topmost_line);

        if let Some(line) = process_line(logical_line) {
            lines.push(line);
        }

        current_line = logical_line_start - 1;
    }

    lines.reverse();
    lines
}
pub(crate) fn update_vi_cursor_for_scroll(term: &mut Term<ZedListener>, scroll: Scroll) {
    match scroll {
        Scroll::Delta(delta) => {
            term.vi_mode_cursor = term.vi_mode_cursor.scroll(term, delta);
        }
        Scroll::PageUp => {
            let lines = term.screen_lines() as i32;
            term.vi_mode_cursor = term.vi_mode_cursor.scroll(term, lines);
        }
        Scroll::PageDown => {
            let lines = -(term.screen_lines() as i32);
            term.vi_mode_cursor = term.vi_mode_cursor.scroll(term, lines);
        }
        Scroll::Top => {
            let point = AlacPoint::new(term.topmost_line(), Column(0));
            term.vi_mode_cursor = ViModeCursor::new(point);
        }
        Scroll::Bottom => {
            let point = AlacPoint::new(term.bottommost_line(), Column(0));
            term.vi_mode_cursor = ViModeCursor::new(point);
        }
    }
}
pub(crate) fn update_selection_to_vi_cursor(term: &mut Term<ZedListener>) -> Option<Point> {
    let mut selection = term.selection.take()?;
    let point = term.vi_mode_cursor.point;
    selection.update(point, AlacDirection::Right);
    term.selection = Some(selection);
    Some(terminal_point_from_alacritty(point))
}
pub(crate) unsafe fn append_text_to_term(term: &mut Term<ZedListener>, text_lines: &[&str]) {
    term.newline();
    term.grid_mut().cursor.point.column = Column(0);
    for line in text_lines {
        for character in line.chars() {
            term.input(character);
        }
        term.newline();
        term.grid_mut().cursor.point.column = Column(0);
    }
}

// 私有 helpers
fn logical_line_for_row(grid: &Grid<AlacCell>, current: i32, topmost: i32) -> (i32, String) {
    let start = find_logical_line_start(grid, current, topmost);
    let mut line = String::new();
    for row in start..=current {
        line.push_str(&row_to_string(&grid[Line(row)]));
    }
    (start, line)
}
fn find_logical_line_start(grid: &Grid<AlacCell>, current: i32, topmost: i32) -> i32 {
    let mut line_start = current;
    while line_start > topmost {
        let previous_line = Line(line_start - 1);
        let last_cell = &grid[previous_line][Column(grid.columns() - 1)];
        if !last_cell.flags.contains(Flags::WRAPLINE) {
            break;
        }
        line_start -= 1;
    }
    line_start
}
fn row_to_string(row: &Row<AlacCell>) -> String {
    row[..Column(row.len())]
        .iter()
        .map(|cell| cell.c)
        .collect::<String>()
}

fn process_line(line: String) -> Option<String> {
    let trimmed = line.trim_end().to_string();
    if !trimmed.is_empty() {
        Some(trimmed)
    } else {
        None
    }
}
