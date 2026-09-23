//! Status bar: a horizontal toolbar of [`StatusItemView`] widgets anchored to
//! the bottom of a workspace.

mod impls;
mod item;
mod render;
mod sidebar_status;
mod status_bar;

pub use item::{HideStatusItem, StatusItemView};
pub use sidebar_status::SidebarStatus;
pub use status_bar::StatusBar;

pub(crate) use item::StatusItemViewHandle;
pub(crate) use render::add_hide_button_entry;
