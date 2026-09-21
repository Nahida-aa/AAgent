use gpui::{App, Axis, Pixels, Window};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::RESIZE_HANDLE_SIZE;
use super::panel::PanelHandle;
use super::position::DockPosition;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PanelSizeState {
    pub size: Option<Pixels>,
    #[serde(default)]
    pub flex: Option<f32>,
}

pub(super) struct PanelEntry {
    pub(super) panel: Arc<dyn PanelHandle>,
    pub(super) size_state: PanelSizeState,
    pub(super) _subscriptions: [gpui::Subscription; 4],
}

pub(super) fn panel_uses_flexible_width(
    position: DockPosition,
    panel: &dyn PanelHandle,
    window: &Window,
    cx: &App,
) -> bool {
    position.axis() == Axis::Horizontal && panel.has_flexible_size(window, cx)
}

pub(super) fn resize_panel_entry(
    position: DockPosition,
    entry: &mut PanelEntry,
    size: Option<Pixels>,
    flex: Option<f32>,
    window: &mut Window,
    cx: &mut App,
) -> (&'static str, PanelSizeState) {
    let size = size.map(|size| size.max(RESIZE_HANDLE_SIZE).round());
    let uses_flexible_width = panel_uses_flexible_width(position, entry.panel.as_ref(), window, cx);
    if uses_flexible_width {
        entry.size_state.flex = flex;
    } else {
        entry.size_state.size = size;
    }
    entry.panel.size_state_changed(window, cx);
    (entry.panel.panel_key(), entry.size_state)
}
