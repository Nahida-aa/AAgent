// ├── registries/
// │   ├── mod.rs
// │   ├── project_items.rs       # ProjectItemRegistry
// │   ├── followable_views.rs    # FollowableViewRegistry
// │   └── serializable_items.rs  # SerializableItemRegistry

use super::*;
mod followable_view;
mod project_item;
mod serializable_item;

pub use followable_view::FollowableViewRegistry;
pub use project_item::{ProjectItemRegistry, register_project_item};
pub(crate) use serializable_item::{SerializableItemRegistry, register_serializable_item};
