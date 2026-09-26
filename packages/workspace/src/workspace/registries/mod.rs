// ├── registries/
// │   ├── mod.rs
// │   ├── project_items.rs       # ProjectItemRegistry
// │   ├── followable_views.rs    # FollowableViewRegistry
// │   └── serializable_items.rs  # SerializableItemRegistry

use super::*;
mod followable_view;
pub(crate) mod project_item;
pub mod serializable_item;

pub use followable_view::FollowableViewRegistry;
pub(crate) use project_item::ProjectItemRegistry;
pub use project_item::register_project_item;
pub(crate) use serializable_item::SerializableItemRegistry;
pub use serializable_item::register_serializable_item;
