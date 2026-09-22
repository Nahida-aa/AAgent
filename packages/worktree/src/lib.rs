//! A worktree: a set of local or remote files opened as part of a project.
//!
//! The [`Worktree`] enum has two variants:
//! - [`Worktree::Local`] tracks filesystem events and git state on disk.
//! - [`Worktree::Remote`] mirrors a worktree owned by a remote host over RPC.
//!
//! Both expose a [`Snapshot`] of the entries they track.

mod ignore;
mod worktree;
mod worktree_settings;

pub use ignore::{IgnoreKind, IgnoreStack};
pub use worktree::*;
pub use worktree_settings::WorktreeSettings;

pub use settings::WorktreeId;
