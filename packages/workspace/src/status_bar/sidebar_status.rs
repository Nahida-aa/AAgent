use gpui::{App, WeakEntity};

use crate::{MultiWorkspace, SidebarSide};

#[derive(Default)]
pub struct SidebarStatus {
    pub open: bool,
    pub side: SidebarSide,
    pub has_notifications: bool,
    pub show_toggle: bool,
}

impl SidebarStatus {
    pub fn query(multi_workspace: &Option<WeakEntity<MultiWorkspace>>, cx: &App) -> Self {
        multi_workspace
            .as_ref()
            .and_then(|mw| mw.upgrade())
            .map(|mw| {
                let mw = mw.read(cx);
                let enabled = mw.multi_workspace_enabled(cx);
                Self {
                    open: mw.sidebar_open() && enabled,
                    side: mw.sidebar_side(cx),
                    has_notifications: mw.sidebar_has_notifications(cx),
                    show_toggle: enabled,
                }
            })
            .unwrap_or_default()
    }
}
