use alacritty_terminal::{
    grid::{Dimensions as _, Grid, Row, Scroll as AlacScroll},
    index::{Column, Line, Point as AlacPoint},
    term::{
        Term,
        cell::{Cell as AlacCell, Flags},
    },
};

use crate::{
    Content, GridLinesChange, HoveredWord, Point, Range, cell::IndexedCell, cursor::Cursor,
};

use super::conversions::*;
use super::types::ZedListener;

pub(crate) fn make_content(term: &Term<ZedListener>, last_content: &Content) -> Content {
    let content = term.renderable_content();

    let estimated_size = content.display_iter.size_hint().0;
    let mut cells = Vec::with_capacity(estimated_size);

    cells.extend(content.display_iter.map(|ic| IndexedCell {
        point: terminal_point_from_alacritty(ic.point),
        cell: terminal_cell_from_alacritty(ic.cell),
    }));

    let selection_text = if content.selection.is_some() {
        term.selection_to_string()
    } else {
        None
    };

    let grid = term.grid();
    let (last_hovered_word, grid_lines_change) = adjusted_last_hovered_word(grid, last_content);

    let bottom_line = term.screen_lines() as i32 - 1 - content.display_offset as i32;
    let bottom_row_occupied = content.cursor.point.line.0 >= bottom_line
        || cells
            .iter()
            .rev()
            .take_while(|cell| cell.point.line >= bottom_line)
            .any(|cell| cell.cell.character() != ' ');

    Content {
        cells,
        mode: terminal_modes_from_alacritty(content.mode),
        total_lines: grid.total_lines(),
        display_offset: grid.display_offset(),
        columns: grid.columns(),
        screen_lines: grid.screen_lines(),
        selection_text,
        selection: content
            .selection
            .map(terminal_selection_range_from_alacritty),
        cursor: Cursor::from_alacritty(content.cursor),
        cursor_char: term.grid()[content.cursor.point].c,
        terminal_bounds: last_content.terminal_bounds,
        last_hovered_word,
        grid_lines_change,
        scrolled_to_top: content.display_offset == term.history_size(),
        scrolled_to_bottom: content.display_offset == 0,
        bottom_row_occupied,
    }
}

fn adjusted_last_hovered_word(
    grid: &Grid<AlacCell>,
    last_content: &Content,
) -> (Option<HoveredWord>, GridLinesChange) {
    let grid_size_changed =
        grid.columns() != last_content.columns || grid.screen_lines() != last_content.screen_lines;

    let Some(total_lines_delta) = grid
        .total_lines()
        .checked_signed_diff(last_content.total_lines)
    else {
        return (None, GridLinesChange::Unchanged);
    };

    let Some(display_offset_delta) = grid
        .display_offset()
        .checked_signed_diff(last_content.display_offset)
    else {
        return (None, GridLinesChange::Unchanged);
    };

    let grid_visible_lines_changed = total_lines_delta != display_offset_delta;

    let grid_lines_change = if grid_size_changed || grid_visible_lines_changed {
        GridLinesChange::Changed
    } else {
        GridLinesChange::Unchanged
    };

    let hovered_word = if let Some(last_hovered_word) = last_content.last_hovered_word.as_ref()
        && grid_lines_change == GridLinesChange::Unchanged
    {
        if total_lines_delta == 0 {
            // Viewport and content are the same, we can reuse existing match
            Some(last_hovered_word.clone())
        } else {
            // Content is changed, but viewport is shifted the same, reuse
            // the match, but need to shift the match lines by the same amount.
            let mut adjusted_hovered_word = last_hovered_word.clone();
            adjusted_hovered_word.word_match = Range::new(
                Point::new(
                    last_hovered_word.word_match.start().line - display_offset_delta as i32,
                    last_hovered_word.word_match.start().column,
                ),
                Point::new(
                    last_hovered_word.word_match.end().line - display_offset_delta as i32,
                    last_hovered_word.word_match.end().column,
                ),
            );

            Some(adjusted_hovered_word)
        }
    } else {
        None
    };

    (hovered_word, grid_lines_change)
}
