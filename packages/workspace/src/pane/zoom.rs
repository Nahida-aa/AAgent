use super::*;

use gpui::{App, Context, FocusOutEvent, Window};
use project::Project;
use settings::Settings;
use theme_settings::ThemeSettings;

use super::Pane;
use super::history::{NavigationMode, TagNavigationMode};
use crate::item::{ItemSettings, PreviewTabsSettings};
use crate::workspace_settings::{FocusFollowsMouse, TabBarSettings, WorkspaceSettings};

impl Pane {
    pub fn toggle_zoom(&mut self, _: &ToggleZoom, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_toggle_zoom {
            cx.propagate();
        } else if self.zoomed {
            cx.emit(Event::ZoomOut);
        } else if !self.items.is_empty() {
            if !self.focus_handle.contains_focused(window, cx) {
                cx.focus_self(window);
            }
            cx.emit(Event::ZoomIn);
        }
    }
    pub fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_toggle_zoom {
            cx.propagate();
        } else if !self.zoomed && !self.items.is_empty() {
            if !self.focus_handle.contains_focused(window, cx) {
                cx.focus_self(window);
            }
            cx.emit(Event::ZoomIn);
        }
    }
    pub fn zoom_out(&mut self, _: &ZoomOut, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_toggle_zoom {
            cx.propagate();
        } else if self.zoomed {
            cx.emit(Event::ZoomOut);
        }
    }
    pub fn set_zoomed(&mut self, zoomed: bool, cx: &mut Context<Self>) {
        self.zoomed = zoomed;
        cx.notify();
    }
    pub fn is_zoomed(&self) -> bool { self.zoomed }
}
