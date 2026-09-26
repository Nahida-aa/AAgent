use super::*;
use gpui::{Context, Entity};

use super::Workspace;
use crate::dock::Dock;

impl Workspace {
    // fn dismiss_zoomed_items_to_reveal
   pub(crate) fn dismiss_zoomed_items_to_reveal(
        &mut self,
        dock_to_reveal: Option<DockPosition>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If a center pane is zoomed, unzoom it.
        for pane in &self.panes {
            if pane != &self.active_pane || dock_to_reveal.is_some() {
                pane.update(cx, |pane, cx| pane.set_zoomed(false, cx));
            }
        }

        // If another dock is zoomed, hide it.
        let mut focus_center = false;
        for dock in self.all_docks() {
            dock.update(cx, |dock, cx| {
                if Some(dock.position()) != dock_to_reveal
                    && let Some(panel) = dock.active_panel()
                    && panel.is_zoomed(window, cx)
                {
                    focus_center |= panel.panel_focus_handle(cx).contains_focused(window, cx);
                    dock.set_open(false, window, cx);
                }
            });
        }

        if focus_center {
            self.active_pane
                .update(cx, |pane, cx| window.focus(&pane.focus_handle(cx), cx))
        }

        if self.zoomed_position != dock_to_reveal {
            self.zoomed = None;
            self.zoomed_position = None;
            cx.emit(Event::ZoomChanged);
        }

        cx.notify();
    }



    //
    pub fn toggle_editor_zoom(
        &mut self,
        _: &ToggleEditorZoom,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.zoomed.is_some() {
            self.active_pane.update(cx, |pane, cx| {
                pane.set_zoomed(false, cx);
            });
            self.zoomed = None;
            self.zoomed_position = None;
            cx.emit(Event::ZoomChanged);
        }

        if let Some(maximized) = self.maximized_pane.take() {
            if maximized.upgrade().as_ref() == Some(&self.active_pane) {
                cx.notify();
                return;
            }
        }

        self.maximized_pane = Some(self.active_pane.downgrade());
        window.focus(&self.active_pane.focus_handle(cx), cx);
        cx.notify();
    }
    //
    pub fn is_pane_maximized(&self) -> bool { self.maximized_pane.is_some() }



}
