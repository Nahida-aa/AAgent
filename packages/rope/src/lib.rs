//! Rope crate — 持久化 B-tree 字符串。
//!
//! 对齐 Zed `crates/rope`，精简版：
//! - 用 Zed 的 `sum_tree`（B-tree）做底层存储
//! - Chunk 是叶子节点（最大 CHUNK_SIZE=64 字节）
//! - 只实现常用操作：push_str, len, slice_to_string, to_string

pub mod chunk;
pub mod rope;

pub use chunk::{CHUNK_SIZE, Chunk, ChunkSummary, TextSummary};
pub use rope::Rope;
