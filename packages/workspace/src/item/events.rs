use ui::Color;
use gpui::{AnyView, App, SharedString, Window};

#[derive(Clone, Copy, Debug)]
pub struct SaveOptions {
    pub format: bool,
    pub force_format: bool,
    pub autosave: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            format: true,
            force_format: false,
            autosave: false,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum ItemEvent {
    CloseItem,
    UpdateTab,
    UpdateBreadcrumbs,
    Edit,
}
