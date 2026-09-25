
impl Workspace {

    pub(super) fn update_window_edited(&mut self, window: &mut Window, cx: &mut App) {
        if !self.owns_window_chrome() {
            return;
        }
        let is_edited = self.is_window_edited(cx);
        if is_edited != self.window_edited {
            self.window_edited = is_edited;
            window.set_window_edited(self.window_edited);
        }
    }
    // 它的最终目的是维护“窗口 edited 标记”，函数体结尾就是 self.update_window_edited(window, cx)
    pub(crate) fn update_item_dirty_state(
        &mut self,
        item: &dyn ItemHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        let is_dirty = item.is_dirty(cx);
        let item_id = item.item_id();
        let was_dirty = self.dirty_items.contains_key(&item_id);
        if is_dirty == was_dirty {
            return;
        }
        if was_dirty {
            self.dirty_items.remove(&item_id);
            self.update_window_edited(window, cx);
            return;
        }

        let workspace = self.weak_handle();
        let Some(window_handle) = window.window_handle().downcast::<MultiWorkspace>() else {
            return;
        };
        let on_release_callback = Box::new(move |cx: &mut App| {
            window_handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |workspace, cx| {
                            workspace.dirty_items.remove(&item_id);
                            workspace.update_window_edited(window, cx)
                        })
                        .ok();
                })
                .ok();
        });

        let s = item.on_release(cx, on_release_callback);
        self.dirty_items.insert(item_id, s);
        self.update_window_edited(window, cx);
    }
    // owns_window_chrome / is_window_edited /  refresh_window_state
}
