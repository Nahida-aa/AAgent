pub(crate) fn px_with_ui_font_fallback(val: u32, cx: &Context<Workspace>) -> Pixels {
    if val == 0 {
        ThemeSettings::get_global(cx).ui_font_size(cx)
    } else {
        px(val as f32)
    }
}

pub(crate) fn adjust_active_dock_size_by_px(
    px: Pixels,
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(active_dock) = workspace
        .all_docks()
        .into_iter()
        .find(|dock| dock.focus_handle(cx).contains_focused(window, cx))
    else {
        return;
    };
    let dock = active_dock.read(cx);
    let Some(panel_size) = workspace.dock_size(&dock, window, cx) else {
        return;
    };
    workspace.resize_dock(dock.position(), panel_size + px, window, cx);
}

pub(crate) fn adjust_open_docks_size_by_px(
    px: Pixels,
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let docks = workspace
        .all_docks()
        .into_iter()
        .filter_map(|dock_entity| {
            let dock = dock_entity.read(cx);
            if dock.is_open() {
                let dock_pos = dock.position();
                let panel_size = workspace.dock_size(&dock, window, cx)?;
                Some((dock_pos, panel_size + px))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for (position, new_size) in docks {
        workspace.resize_dock(position, new_size, window, cx);
    }
}

pub(crate) fn notify_if_database_failed(window: WindowHandle<MultiWorkspace>, cx: &mut AsyncApp) {
    window
        .update(cx, |multi_workspace, _, cx| {
            let workspace = multi_workspace.workspace().clone();
            workspace.update(cx, |workspace, cx| {
                if (*db::ALL_FILE_DB_FAILED).load(std::sync::atomic::Ordering::Acquire) {
                    struct DatabaseFailedNotification;

                    workspace.show_notification(
                        NotificationId::unique::<DatabaseFailedNotification>(),
                        cx,
                        |cx| {
                            cx.new(|cx| {
                                MessageNotification::new("Failed to load the database file.", cx)
                                    .primary_message("File an Issue")
                                    .primary_icon(IconName::Plus)
                                    .primary_on_click(|window, cx| {
                                        window.dispatch_action(Box::new(FileBugReport), cx)
                                    })
                            })
                        },
                    );
                }
            });
        })
        .log_err();
}

pub(crate) fn serialize_pane_handle(
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

pub(crate) fn leader_border_for_pane(
    follower_states: &HashMap<CollaboratorId, FollowerState>,
    pane: &Entity<Pane>,
    _: &Window,
    cx: &App,
) -> Option<Div> {
    let (leader_id, _follower_state) = follower_states.iter().find_map(|(leader_id, state)| {
        if state.pane() == pane {
            Some((*leader_id, state))
        } else {
            None
        }
    })?;

    let mut leader_color = match leader_id {
        CollaboratorId::PeerId(leader_peer_id) => {
            let leader = GlobalAnyActiveCall::try_global(cx)?
                .0
                .remote_participant_for_peer_id(leader_peer_id, cx)?;

            cx.theme()
                .players()
                .color_for_participant(leader.participant_index.0)
                .cursor
        }
        CollaboratorId::Agent => cx.theme().players().agent().cursor,
    };
    leader_color.fade_out(0.3);
    Some(
        div()
            .absolute()
            .size_full()
            .left_0()
            .top_0()
            .border_2()
            .border_color(leader_color),
    )
}

pub(crate) fn join_pane_into_active(
    active_pane: &Entity<Pane>,
    pane: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) {
    if pane == active_pane {
    } else if pane.read(cx).items_len() == 0 {
        pane.update(cx, |_, cx| {
            cx.emit(pane::Event::Remove {
                focus_on_pane: None,
            });
        })
    } else {
        move_all_items(pane, active_pane, window, cx);
    }
}

pub(crate) fn move_all_items(
    from_pane: &Entity<Pane>,
    to_pane: &Entity<Pane>,
    window: &mut Window,
    cx: &mut App,
) {
    let destination_is_different = from_pane != to_pane;
    let mut moved_items = 0;
    for (item_ix, item_handle) in from_pane
        .read(cx)
        .items()
        .enumerate()
        .map(|(ix, item)| (ix, item.clone()))
        .collect::<Vec<_>>()
    {
        let ix = item_ix - moved_items;
        if destination_is_different {
            // Close item from previous pane
            from_pane.update(cx, |source, cx| {
                source.remove_item_and_focus_on_pane(ix, false, to_pane.clone(), window, cx);
            });
            moved_items += 1;
        }

        // This automatically removes duplicate items in the pane
        to_pane.update(cx, |destination, cx| {
            destination.add_item(item_handle, true, true, None, window, cx);
            window.focus(&destination.focus_handle(cx), cx)
        });
    }
}

pub fn move_item(
    source: &Entity<Pane>,
    destination: &Entity<Pane>,
    item_id_to_move: EntityId,
    destination_index: usize,
    activate: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((item_ix, item_handle)) = source
        .read(cx)
        .items()
        .enumerate()
        .find(|(_, item_handle)| item_handle.item_id() == item_id_to_move)
        .map(|(ix, item)| (ix, item.clone()))
    else {
        // Tab was closed during drag
        return;
    };

    if source != destination {
        // Close item from previous pane
        source.update(cx, |source, cx| {
            source.remove_item_and_focus_on_pane(item_ix, false, destination.clone(), window, cx);
        });
    }

    // This automatically removes duplicate items in the pane
    destination.update(cx, |destination, cx| {
        destination.add_item_inner(
            item_handle,
            activate,
            activate,
            activate,
            Some(destination_index),
            window,
            cx,
        );
        if activate {
            window.focus(&destination.focus_handle(cx), cx)
        }
    });
}

pub fn move_active_item(
    source: &Entity<Pane>,
    destination: &Entity<Pane>,
    focus_destination: bool,
    close_if_empty: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if source == destination {
        return;
    }
    let Some(active_item) = source.read(cx).active_item() else {
        return;
    };
    source.update(cx, |source_pane, cx| {
        let item_id = active_item.item_id();
        source_pane.remove_item(item_id, false, close_if_empty, window, cx);
        destination.update(cx, |target_pane, cx| {
            target_pane.add_item(
                active_item,
                focus_destination,
                focus_destination,
                Some(target_pane.items_len()),
                window,
                cx,
            );
        });
    });
}

/// Reads a panel's pixel size from its legacy KVP format and deletes the legacy
/// key. This migration path only runs once per panel per workspace.
fn load_legacy_panel_size(
    panel_key: &str,
    dock_position: DockPosition,
    workspace: &Workspace,
    cx: &mut App,
) -> Option<Pixels> {
    #[derive(Deserialize)]
    struct LegacyPanelState {
        #[serde(default)]
        width: Option<Pixels>,
        #[serde(default)]
        height: Option<Pixels>,
    }

    let workspace_id = workspace
        .database_id()
        .map(|id| i64::from(id).to_string())
        .or_else(|| workspace.session_id())?;

    let legacy_key = match panel_key {
        "ProjectPanel" => {
            format!("{}-{:?}", "ProjectPanel", workspace_id)
        }
        "OutlinePanel" => {
            format!("{}-{:?}", "OutlinePanel", workspace_id)
        }
        "GitPanel" => {
            format!("{}-{:?}", "GitPanel", workspace_id)
        }
        "TerminalPanel" => {
            format!("{:?}-{:?}", "TerminalPanel", workspace_id)
        }
        _ => return None,
    };

    let kvp = db::kvp::KeyValueStore::global(cx);
    let json = kvp.read_kvp(&legacy_key).log_err().flatten()?;
    let state = serde_json::from_str::<LegacyPanelState>(&json).log_err()?;
    let size = match dock_position {
        DockPosition::Bottom => state.height,
        DockPosition::Left | DockPosition::Right => state.width,
    }?;

    cx.background_spawn(async move { kvp.delete_kvp(legacy_key).await })
        .detach_and_log_err(cx);

    Some(size)
}
