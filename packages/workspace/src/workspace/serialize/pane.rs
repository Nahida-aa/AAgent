fn serialize_pane_handle(
    pane_handle: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) -> SerializedPane {
    let (items, active, pinned_count) = {
        let pane = pane_handle.read(cx);
        let active_item_id = pane.active_item().map(|item| item.item_id());
        // Pinned tabs are the leading tabs of a pane, so the pinned count has to
        // shrink along with every pinned item that is dropped here. Otherwise a
        // tab that was not pinned would take the dropped item's slot and come
        // back pinned on the next restore.
        let pinned_region = 0..pane.pinned_count();
        let mut pinned_count = pane.pinned_count();
        let items = pane
            .items()
            .enumerate()
            .filter_map(|(index, handle)| {
                let Some(handle) = handle.to_serializable_item_handle(cx) else {
                    if pinned_region.contains(&index) {
                        pinned_count -= 1;
                    }
                    return None;
                };

                Some(SerializedItem {
                    kind: Arc::from(handle.serialized_item_kind()),
                    item_id: handle.item_id().as_u64(),
                    active: Some(handle.item_id()) == active_item_id,
                    preview: pane.is_active_preview_item(handle.item_id()),
                })
            })
            .collect::<Vec<_>>();

        (items, pane.has_focus(window, cx), pinned_count)
    };

    SerializedPane::new(items, active, pinned_count)
}
