//! 精简版 Chunk — rope 的叶子节点。
//!
//! 对齐 Zed `crates/rope/src/chunk.rs`，但去掉了 bitmap 优化（chars/chars_utf16/newlines/tabs），
//! 只用简单 String + 预计算 TextSummary。bitmap 版 1200 行，我们约 60 行——够用了。

use heapless::String as ArrayString;
use sum_tree::{ContextLessSummary, Dimension, Item, Summary};

/// Chunk 最大字节数（Zed 是 128，用 bitmap 编码；我们用更小以适配 heapless）。
pub const CHUNK_SIZE: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub text: ArrayString<CHUNK_SIZE>,
}

impl Chunk {
    pub fn new(text: &str) -> Self {
        let mut buf = ArrayString::new();
        let take = text.len().min(CHUNK_SIZE);
        let mut end = take;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        buf.push_str(&text[..end]).unwrap();
        Self { text: buf }
    }

    pub fn push_str(&mut self, text: &str) {
        let mut remaining = text;
        while !remaining.is_empty() && self.text.len() < CHUNK_SIZE {
            let space = CHUNK_SIZE - self.text.len();
            let take = remaining.len().min(space);
            let mut end = take;
            while end > 0 && !remaining.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                break;
            }
            self.text.push_str(&remaining[..end]).unwrap();
            remaining = &remaining[end..];
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }
}

// ---------- TextSummary ----------

/// 文本维度统计 — SumTree 用它来合并子树 summary。
///
/// 对齐 Zed `crates/rope/src/rope.rs::TextSummary`，精简版。
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct TextSummary {
    pub len: usize,
    pub chars: usize,
    pub lines: usize,
    pub last_line_len: usize,
}

impl TextSummary {
    pub fn from_str(text: &str) -> Self {
        let mut summary = Self::default();
        summary.len = text.len();
        summary.chars = text.chars().count();
        // 计算行数 + 最后一行长度
        let mut rows = 0usize;
        let mut last = 0usize;
        for c in text.chars() {
            if c == '\n' {
                rows += 1;
                last = 0;
            } else {
                last += c.len_utf8();
            }
        }
        summary.lines = rows;
        summary.last_line_len = last;
        summary
    }
}

impl std::ops::AddAssign<&Self> for TextSummary {
    fn add_assign(&mut self, rhs: &Self) {
        self.len += rhs.len;
        self.chars += rhs.chars;
        if rhs.lines == 0 {
            self.last_line_len += rhs.last_line_len;
        } else {
            self.lines += rhs.lines;
            self.last_line_len = rhs.last_line_len;
        }
    }
}

// ---------- sum_tree impl ----------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChunkSummary {
    pub text: TextSummary,
}

impl ContextLessSummary for ChunkSummary {
    fn zero() -> Self {
        Default::default()
    }

    fn add_summary(&mut self, other: &Self) {
        self.text += &other.text;
    }
}

impl Item for Chunk {
    type Summary = ChunkSummary;

    fn summary(&self, _cx: <Self::Summary as Summary>::Context<'_>) -> Self::Summary {
        ChunkSummary {
            text: TextSummary::from_str(&self.text),
        }
    }
}

/// 让 TextSummary 可以作为维度（按字节 offset 导航）
impl<'a> Dimension<'a, ChunkSummary> for TextSummary {
    fn zero(_cx: <ChunkSummary as Summary>::Context<'_>) -> Self {
        Default::default()
    }

    fn add_summary(
        &mut self,
        summary: &'a ChunkSummary,
        _cx: <ChunkSummary as Summary>::Context<'_>,
    ) {
        *self += &summary.text;
    }
}

/// 让 usize 可以作为维度（按字节数导航）
impl<'a> Dimension<'a, ChunkSummary> for usize {
    fn zero(_cx: <ChunkSummary as Summary>::Context<'_>) -> Self {
        0
    }

    fn add_summary(
        &mut self,
        summary: &'a ChunkSummary,
        _cx: <ChunkSummary as Summary>::Context<'_>,
    ) {
        *self += summary.text.len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_new() {
        let c = Chunk::new("hello world");
        assert_eq!(c.as_str(), "hello world");
        assert_eq!(c.len(), 11);
    }

    #[test]
    fn test_chunk_push_str() {
        let mut c = Chunk::new("abc");
        c.push_str("def");
        assert_eq!(c.as_str(), "abcdef");
    }

    #[test]
    fn test_text_summary() {
        let s = TextSummary::from_str("hello\nworld\n");
        assert_eq!(s.len, 12);
        assert_eq!(s.chars, 12);
        assert_eq!(s.lines, 2);
        assert_eq!(s.last_line_len, 0);

        let s = TextSummary::from_str("hello\nworld");
        assert_eq!(s.lines, 1);
        assert_eq!(s.last_line_len, 5);
    }

    #[test]
    fn test_chunk_summary_merge() {
        let a = Chunk::new("hello\n");
        let b = Chunk::new("world");
        let sa = a.summary(());
        let sb = b.summary(());
        let mut merged = <ChunkSummary as ContextLessSummary>::zero();
        ContextLessSummary::add_summary(&mut merged, &sa);
        ContextLessSummary::add_summary(&mut merged, &sb);
        assert_eq!(merged.text.len, 11);
        assert_eq!(merged.text.lines, 1);
        assert_eq!(merged.text.last_line_len, 5);
    }
}
