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

#[derive(Clone, Copy, Default, Debug)]
pub struct TabContentParams {
    pub detail: Option<usize>,
    pub selected: bool,
    pub preview: bool,
    /// Tab content should be deemphasized when active pane does not have focus.
    pub deemphasized: bool,
    /// Maximum character length for the title. None = use the item's own default (typically MAX_TAB_TITLE_LEN).
    pub max_title_len: Option<usize>,
    pub truncate_title_middle: bool,
}

impl TabContentParams {
    /// Returns the text color to be used for the tab content.
    pub fn text_color(&self) -> Color {
        if self.deemphasized {
            if self.selected {
                Color::Muted
            } else {
                Color::Hidden
            }
        } else if self.selected {
            Color::Default
        } else {
            Color::Muted
        }
    }
}

pub enum TabTooltipContent {
    Text(SharedString),
    Custom(Box<dyn Fn(&mut Window, &mut App) -> AnyView>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemBufferKind {
    Multibuffer,
    Singleton,
    None,
}
