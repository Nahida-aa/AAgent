use super::Workspace;
use crate::{dock::Dock, workspace::event::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

pub mod flush;
// WorkspaceLocation, workspace_location
pub mod workspace;

pub const SERIALIZATION_THROTTLE_TIME: Duration = Duration::from_millis(200);
