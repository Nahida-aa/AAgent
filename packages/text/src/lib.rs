//! Text primitives for Zed.
//!
//! This crate provides:
//!
//! - [`Buffer`] / [`BufferSnapshot`]: a concurrent, rope-backed editable text
//!   buffer with anchors and undo/redo history.
//! - The [`ToOffset`] / [`ToPoint`] family of conversions between offsets,
//!   `Point`s, and UTF-16 `Point`s.
//! - Small utility modules (`patch`, `selection`, `subscription`, `undo_map`,
//!   `operation_queue`, `locator`) reused across the rest of the editor.
//!
//! The rope implementation itself lives in the external `rope` crate and is
//! re-exported here for convenience.

mod anchor;
mod buffer;
pub mod locator;
#[cfg(any(test, feature = "test-support"))]
pub mod network;
pub mod operation_queue;
mod patch;
mod selection;
pub mod subscription;
mod undo_map;

#[cfg(test)]
mod tests;

pub use anchor::*;
pub use buffer::*;
pub use patch::Patch;
pub use rope::*;
pub use selection::*;
pub use subscription::*;
pub use sum_tree::Bias;

// `ReplicaId` is used pervasively alongside `Buffer`, so keep it available at
// the crate root as well.
pub use aa_clock::ReplicaId;
