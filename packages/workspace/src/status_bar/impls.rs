use std::any::TypeId;

use gpui::{AnyView, App, Entity, Window};

use crate::ItemHandle;

use super::item::{HideStatusItem, StatusItemView, StatusItemViewHandle};

impl<T: StatusItemView> StatusItemViewHandle for Entity<T> {
    fn to_any(&self) -> AnyView { self.clone().into() }

    fn set_active_pane_item(
        &self,
        active_pane_item: Option<&dyn ItemHandle>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.update(cx, |this, cx| {
            this.set_active_pane_item(active_pane_item, window, cx)
        });
    }

    fn item_type(&self) -> TypeId { TypeId::of::<T>() }

    fn hide_setting(&self, cx: &App) -> Option<HideStatusItem> { self.read(cx).hide_setting(cx) }
}

impl From<&dyn StatusItemViewHandle> for AnyView {
    fn from(val: &dyn StatusItemViewHandle) -> Self { val.to_any() }
}
