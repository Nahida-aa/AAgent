use super::*;

use std::ops;

use crate::chunk::ChunkSlice;
use std::{
    cmp, fmt, io, mem,
    ops::{self, AddAssign, Range},
    str,
};
use sum_tree::Item;

/// 由单个 [`Chunk`] 产出的摘要
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChunkSummary {
    pub(crate) text: TextSummary,
}

impl sum_tree::Item for Chunk {
    type Summary = ChunkSummary;

    fn summary(&self, _cx: ()) -> Self::Summary {
        ChunkSummary {
            text: self.as_slice().text_summary(),
        }
    }
}

impl sum_tree::ContextLessSummary for ChunkSummary {
    fn zero() -> Self { Default::default() }
    fn add_summary(&mut self, summary: &Self) { self.text += &summary.text; }
}

/// Summary of a string of text.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TextSummary {
    pub len: usize,
    pub chars: usize,
    pub len_utf16: OffsetUtf16,
    pub lines: Point,
    pub first_line_chars: u32,
    pub last_line_chars: u32,
    pub last_line_len_utf16: u32,
    pub longest_row: u32,
    pub longest_row_chars: u32,
}

impl TextSummary {
    pub fn lines_utf16(&self) -> PointUtf16 {
        PointUtf16 {
            row: self.lines.row,
            column: self.last_line_len_utf16,
        }
    }

    pub fn newline() -> Self {
        Self {
            len: 1,
            chars: 1,
            len_utf16: OffsetUtf16(1),
            first_line_chars: 0,
            last_line_chars: 0,
            last_line_len_utf16: 0,
            lines: Point::new(1, 0),
            longest_row: 0,
            longest_row_chars: 0,
        }
    }

    pub fn add_newline(&mut self) {
        self.len += 1;
        self.len_utf16 += OffsetUtf16(self.len_utf16.0 + 1);
        self.last_line_chars = 0;
        self.last_line_len_utf16 = 0;
        self.lines += Point::new(1, 0);
    }
}

impl<'a> From<&'a str> for TextSummary {
    fn from(text: &'a str) -> Self {
        let mut len_utf16 = OffsetUtf16(0);
        let mut lines = Point::new(0, 0);
        let mut first_line_chars = 0;
        let mut last_line_chars = 0;
        let mut last_line_len_utf16 = 0;
        let mut longest_row = 0;
        let mut longest_row_chars = 0;
        let mut chars = 0;
        for c in text.chars() {
            chars += 1;
            len_utf16.0 += c.len_utf16();

            if c == '\n' {
                lines += Point::new(1, 0);
                last_line_len_utf16 = 0;
                last_line_chars = 0;
            } else {
                lines.column += c.len_utf8() as u32;
                last_line_len_utf16 += c.len_utf16() as u32;
                last_line_chars += 1;
            }

            if lines.row == 0 {
                first_line_chars = last_line_chars;
            }

            if last_line_chars > longest_row_chars {
                longest_row = lines.row;
                longest_row_chars = last_line_chars;
            }
        }

        TextSummary {
            len: text.len(),
            chars,
            len_utf16,
            lines,
            first_line_chars,
            last_line_chars,
            last_line_len_utf16,
            longest_row,
            longest_row_chars,
        }
    }
}

impl sum_tree::ContextLessSummary for TextSummary {
    fn zero() -> Self { Default::default() }
    fn add_summary(&mut self, summary: &Self) { *self += summary; }
}

impl ops::Add<Self> for TextSummary {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        AddAssign::add_assign(&mut self, &rhs);
        self
    }
}

impl<'a> ops::AddAssign<&'a Self> for TextSummary {
    fn add_assign(&mut self, other: &'a Self) {
        let joined_chars = self.last_line_chars + other.first_line_chars;
        if joined_chars > self.longest_row_chars {
            self.longest_row = self.lines.row;
            self.longest_row_chars = joined_chars;
        }
        if other.longest_row_chars > self.longest_row_chars {
            self.longest_row = self.lines.row + other.longest_row;
            self.longest_row_chars = other.longest_row_chars;
        }

        if self.lines.row == 0 {
            self.first_line_chars += other.first_line_chars;
        }

        if other.lines.row == 0 {
            self.last_line_chars += other.first_line_chars;
            self.last_line_len_utf16 += other.last_line_len_utf16;
        } else {
            self.last_line_chars = other.last_line_chars;
            self.last_line_len_utf16 = other.last_line_len_utf16;
        }

        self.chars += other.chars;
        self.len += other.len;
        self.len_utf16 += other.len_utf16;
        self.lines += other.lines;
    }
}

impl ops::AddAssign<Self> for TextSummary {
    fn add_assign(&mut self, other: Self) { *self += &other; }
}
