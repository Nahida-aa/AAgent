use super::*;

pub(crate) fn contiguous_ranges(
    values: impl Iterator<Item = u32>,
    max_len: usize,
) -> impl Iterator<Item = Range<u32>> {
    let mut values = values;
    let mut current_range: Option<Range<u32>> = None;
    std::iter::from_fn(move || {
        loop {
            if let Some(value) = values.next() {
                if let Some(range) = &mut current_range
                    && value == range.end
                    && range.len() < max_len
                {
                    range.end += 1;
                    continue;
                }

                let prev_range = current_range.clone();
                current_range = Some(value..(value + 1));
                if prev_range.is_some() {
                    return prev_range;
                }
            } else {
                return current_range.take();
            }
        }
    })
}

pub(crate) fn offset_in_sub_ranges(
    sub_ranges: &[Range<Anchor>],
    offset: usize,
    snapshot: &TextBufferSnapshot,
) -> bool {
    let start_anchor = snapshot.anchor_before(offset);
    let end_anchor = snapshot.anchor_after(offset);

    sub_ranges.iter().any(|sub_range| {
        let is_before_start = sub_range.end.cmp(&start_anchor, snapshot).is_lt();
        let is_after_end = sub_range.start.cmp(&end_anchor, snapshot).is_gt();
        !is_before_start && !is_after_end
    })
}

pub(crate) fn trailing_whitespace_ranges(
    rope: &Rope,
    row_ranges: Option<&[Range<u32>]>,
) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();

    let is_row_included = |row: u32| match row_ranges {
        Some(row_ranges) => row_ranges.iter().any(|range| range.contains(&row)),
        None => true,
    };

    let mut offset = 0;
    let mut current_row: u32 = 0;
    let mut prev_row: Option<u32> = None;
    let mut prev_chunk_trailing_whitespace_range = 0..0;
    for chunk in rope.chunks() {
        let mut prev_line_trailing_whitespace_range = 0..0;
        for (i, line) in chunk.split('\n').enumerate() {
            let line_end_offset = offset + line.len();
            let trimmed_line_len = line.trim_end_matches([' ', '\t']).len();
            let mut trailing_whitespace_range = (offset + trimmed_line_len)..line_end_offset;

            if i == 0 && trimmed_line_len == 0 {
                trailing_whitespace_range.start = prev_chunk_trailing_whitespace_range.start;
            }
            if let Some(row) = prev_row {
                if !prev_line_trailing_whitespace_range.is_empty() && is_row_included(row) {
                    ranges.push(prev_line_trailing_whitespace_range);
                }
            }

            prev_row = Some(current_row);
            offset = line_end_offset + 1;
            current_row += 1;
            prev_line_trailing_whitespace_range = trailing_whitespace_range;
        }

        offset -= 1;
        current_row -= 1;
        prev_chunk_trailing_whitespace_range = prev_line_trailing_whitespace_range;
    }

    if !prev_chunk_trailing_whitespace_range.is_empty() && is_row_included(current_row) {
        ranges.push(prev_chunk_trailing_whitespace_range);
    }

    ranges
}
