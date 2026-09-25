use gpui::{App, FocusHandle, Window};

use crate::dock::Dock;
use crate::pane::Pane;
use crate::types::DockPosition;

/// A focusable major window region used by region navigation.
pub(crate) struct FocusablePart {
    pub(crate) container: FocusHandle,
    pub(crate) behavior: PartBehavior,
}

pub(crate) enum PartBehavior {
    Toolbar,
    Landmark { content: FocusHandle },
}

impl FocusablePart {
    fn toolbar(container: FocusHandle) -> Self {
        Self {
            container,
            behavior: PartBehavior::Toolbar,
        }
    }

    fn landmark(wrapper: FocusHandle, content: FocusHandle) -> Self {
        Self {
            container: wrapper,
            behavior: PartBehavior::Landmark { content },
        }
    }

    /// Whether focus currently lies within this region. Checks both the wrapper
    /// (focused/ancestor when a screen reader is active) and, for landmarks, the
    /// interactive content (focused when a screen reader is not active, since
    /// the wrapper isn't in the focus tree then).
    fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        if self.container.contains_focused(window, cx) {
            return true;
        }
        match &self.behavior {
            PartBehavior::Landmark { content } => content.contains_focused(window, cx),
            PartBehavior::Toolbar => false,
        }
    }
}

/// Focus handles for the landmark region wrappers.
pub(crate) struct RegionFocusHandles {
    pub(crate) left_dock: FocusHandle,
    pub(crate) right_dock: FocusHandle,
    pub(crate) bottom_dock: FocusHandle,
    pub(crate) editor: FocusHandle,
}

impl RegionFocusHandles {
    fn new(cx: &mut App) -> Self {
        Self {
            left_dock: cx.focus_handle(),
            right_dock: cx.focus_handle(),
            bottom_dock: cx.focus_handle(),
            editor: cx.focus_handle(),
        }
    }

    fn dock(&self, position: DockPosition) -> &FocusHandle {
        match position {
            DockPosition::Left => &self.left_dock,
            DockPosition::Right => &self.right_dock,
            DockPosition::Bottom => &self.bottom_dock,
        }
    }
}

pub(crate) enum ActivateInDirectionTarget {
    Pane(gpui::Entity<Pane>),
    Dock(gpui::Entity<Dock>),
    Sidebar(FocusHandle),
}
