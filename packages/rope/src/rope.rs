//! Rope — 持久化 B-tree 字符串。
//!
//! 对齐 Zed `crates/rope/src/rope.rs`，精简版：
//! - 用 Zed 的 sum_tree 做底层存储
//! - Chunk 是叶子（最大 CHUNK_SIZE=64 字节）
//! - 只实现最常用的操作：push_str, len, slice_to_string, is_empty

use sum_tree::{Bias, SumTree};

use crate::chunk::{CHUNK_SIZE, Chunk, TextSummary};

#[derive(Clone, Default)]
pub struct Rope {
    chunks: SumTree<Chunk>,
}

impl Rope {
    pub fn new() -> Self {
        Self {
            chunks: SumTree::new(()),
        }
    }

    /// 从字符串构建 Rope。
    pub fn from_str(text: &str) -> Self {
        let mut rope = Self::new();
        rope.push_str(text);
        rope
    }

    /// 追加文本。会自动切分 Chunk。
    pub fn push_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // 按 CHUNK_SIZE 切分 text，逐个 push
        let mut remaining = text;
        while !remaining.is_empty() {
            let take = remaining.len().min(CHUNK_SIZE);
            let mut end = take;
            while end > 0 && !remaining.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                break;
            }
            self.chunks.push(Chunk::new(&remaining[..end]), ());
            remaining = &remaining[end..];
        }
    }

    /// 总字节数。
    pub fn len(&self) -> usize {
        self.chunks.summary().text.len
    }

    /// 总字符数。
    pub fn char_count(&self) -> usize {
        self.chunks.summary().text.chars
    }

    /// 总行数。
    pub fn line_count(&self) -> usize {
        let s = self.chunks.summary().text;
        s.lines + if s.last_line_len > 0 { 1 } else { 0 }
    }

    /// 是否空。
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// 转成 String（遍历所有 chunk）。
    pub fn to_string(&self) -> String {
        let mut out = String::with_capacity(self.len());
        let mut cursor = self.chunks.cursor::<TextSummary>(());
        cursor.seek(&TextSummary::default(), Bias::Left);
        while let Some(chunk) = cursor.item() {
            out.push_str(chunk.as_str());
            cursor.next();
        }
        out
    }

    /// 子字符串切片（start..end，按字节 offset）。
    pub fn slice_to_string(&self, start: usize, end: usize) -> String {
        if start >= end || self.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        let mut cursor = self.chunks.cursor::<usize>(());
        let mut offset = 0usize;
        cursor.seek(&0usize, Bias::Left);
        while let Some(chunk) = cursor.item() {
            let chunk_end = offset + chunk.len();
            if chunk_end <= start {
                offset = chunk_end;
                cursor.next();
                continue;
            }
            if offset >= end {
                break;
            }
            let local_start = start.saturating_sub(offset);
            let local_end = (end - offset).min(chunk.len());
            out.push_str(&chunk.as_str()[local_start..local_end]);
            offset = chunk_end;
            cursor.next();
        }
        out
    }

    /// chunk 数量。
    pub fn chunk_count(&self) -> usize {
        let mut cursor = self.chunks.cursor::<TextSummary>(());
        cursor.seek(&TextSummary::default(), Bias::Left);
        let mut count = 0;
        while cursor.item().is_some() {
            count += 1;
            cursor.next();
        }
        count
    }
}

impl std::fmt::Display for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_from_str() {
        let r = Rope::from_str("hello world");
        assert_eq!(r.len(), 11);
        assert_eq!(r.char_count(), 11);
        assert!(!r.is_empty());
        assert_eq!(r.to_string(), "hello world");
    }

    #[test]
    fn test_rope_multiline() {
        let r = Rope::from_str("hello\nworld\nfoo");
        assert_eq!(r.line_count(), 3);
        assert_eq!(r.to_string(), "hello\nworld\nfoo");
    }

    #[test]
    fn test_rope_append_large() {
        let text = "x".repeat(CHUNK_SIZE * 3);
        let r = Rope::from_str(&text);
        assert_eq!(r.len(), CHUNK_SIZE * 3);
        assert!(r.chunk_count() >= 3);
        assert_eq!(r.to_string(), text);
    }

    #[test]
    fn test_rope_slice() {
        let r = Rope::from_str("hello world");
        assert_eq!(r.slice_to_string(0, 5), "hello");
        assert_eq!(r.slice_to_string(6, 11), "world");
        assert_eq!(r.slice_to_string(0, 0), "");
    }

    #[test]
    fn test_rope_slice_cross_chunk() {
        let text = "a".repeat(CHUNK_SIZE * 2 + 10);
        let r = Rope::from_str(&text);
        let mid = CHUNK_SIZE;
        assert_eq!(r.slice_to_string(mid - 5, mid + 5), "a".repeat(10));
    }

    #[test]
    fn test_rope_empty() {
        let r = Rope::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.to_string(), "");
    }

    #[test]
    fn test_rope_unicode() {
        let r = Rope::from_str("你好世界");
        assert_eq!(r.char_count(), 4);
        assert_eq!(r.len(), 12); // 3 bytes each
        assert_eq!(r.to_string(), "你好世界");
    }
}
