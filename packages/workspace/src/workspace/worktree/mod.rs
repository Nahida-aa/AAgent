use super::Workspace;
use super::*;
use crate::{dock::Dock, workspace::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

// active_worktree_creation
mod read;
// set_active_worktree_creation
mod ops;
// workspace/worktree/switch.rs
// capture_state_for_worktree_switch
mod switch;
pub mod trust;
