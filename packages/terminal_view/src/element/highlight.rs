use editor::{HighlightedRange, HighlightedRangeLine};
use gpui::{Bounds, Pixels, Point as GpuiPoint};
use terminal::Range;

use super::layout::{LayoutPoint, LayoutState};

pub(super) fn to_highlighted_range_lines(
    range: &Range,
    layout: &LayoutState,
    origin: GpuiPoint<Pixels>,
) -> Option<(Pixels, Vec<HighlightedRangeLine>)> {
    // Step 1. Normalize the points to be viewport relative.
    // When display_offset = 1, here's how the grid is arranged:
    //-2,0 -2,1...
    //--- Viewport top
    //-1,0 -1,1...
    //--------- Terminal Top
    // 0,0  0,1...
    // 1,0  1,1...
    //--- Viewport Bottom
    // 2,0  2,1...
    //--------- Terminal Bottom

    // Normalize to viewport relative, from terminal relative.
    // lines are i32s, which are negative above the top left corner of the terminal
    // If the user has scrolled, we use the display_offset to tell us which offset
    // of the grid data we should be looking at. But for the rendering step, we don't
    // want negatives. We want things relative to the 'viewport' (the area of the grid
    // which is currently shown according to the display offset)
    let display_offset = i32::try_from(layout.display_offset).unwrap_or(i32::MAX);
    let unclamped_start_line = range.start().line.saturating_add(display_offset);
    let unclamped_start_column = range.start().column;
    let unclamped_end_line = range.end().line.saturating_add(display_offset);
    let unclamped_end_column = range.end().column;

    // Step 2. Clamp range to viewport, and return None if it doesn't overlap
    if unclamped_end_line < 0 || unclamped_start_line > layout.dimensions.num_lines() as i32 {
        return None;
    }

    let clamped_start_line = unclamped_start_line.max(0) as usize;

    let clamped_end_line = unclamped_end_line.min(layout.dimensions.num_lines() as i32) as usize;

    // Convert the start of the range to pixels
    let start_y = origin.y + clamped_start_line as f32 * layout.dimensions.line_height;

    // Step 3. Expand ranges that cross lines into a collection of single-line ranges.
    //  (also convert to pixels)
    let mut highlighted_range_lines = Vec::new();
    for line in clamped_start_line..=clamped_end_line {
        let mut line_start = 0;
        let mut line_end = layout.dimensions.num_columns();

        if line == clamped_start_line && unclamped_start_line >= 0 {
            line_start = unclamped_start_column;
        }
        if line == clamped_end_line && unclamped_end_line <= layout.dimensions.num_lines() as i32 {
            line_end = unclamped_end_column + 1; // +1 for inclusive
        }

        highlighted_range_lines.push(HighlightedRangeLine {
            start_x: origin.x + line_start as f32 * layout.dimensions.cell_width,
            end_x: origin.x + line_end as f32 * layout.dimensions.cell_width,
        });
    }

    Some((start_y, highlighted_range_lines))
}
