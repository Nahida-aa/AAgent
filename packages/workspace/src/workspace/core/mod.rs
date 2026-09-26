// workspace/core/
// ├── mod.rs
// ├── workspace.rs          # Workspace 结构体定义
// ├── ids.rs                # WorkspaceId
// ├── event.rs             # Event + impl EventEmitter<Event>
// ├── debounce.rs           # DelayedDebouncedEditAction
// ├── keystrokes.rs         # DispatchingKeystrokes
// ├── construct.rs          # new / new_local / test_new
// ├── lifecycle.rs          # prepare_to_close / reload / flush / CloseIntent
// ├── read.rs          # weak_handle / app_state / project / ...
// └── actions.rs            # namespace = workspace 的 action

use super::*;
pub mod actions;
pub mod construct;
pub mod debounce;
pub mod event;
mod ids;
pub use ids::WorkspaceId;
pub mod keystrokes;
pub mod lifecycle;
pub mod workspace;
pub(crate) use workspace::Workspace;
pub mod history;
pub mod ops;
pub mod render;
mod read;
pub(crate) use debounce::{DelayedDebouncedEditAction};
pub(crate) use keystrokes::DispatchingKeystrokes;
pub(crate) use lifecycle::CloseIntent;
