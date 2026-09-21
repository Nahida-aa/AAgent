fn px_with_ui_font_fallback(val: u32, cx: &Context<Workspace>) -> Pixels {
    if val == 0 {
        ThemeSettings::get_global(cx).ui_font_size(cx)
    } else {
        px(val as f32)
    }
}

fn adjust_active_dock_size_by_px(
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

fn adjust_open_docks_size_by_px(
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

fn notify_if_database_failed(window: WindowHandle<MultiWorkspace>, cx: &mut AsyncApp) {
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

fn parse_pixel_position_env_var(value: &str) -> Option<Point<Pixels>> {
    let mut parts = value.split(',');
    let x: usize = parts.next()?.parse().ok()?;
    let y: usize = parts.next()?.parse().ok()?;
    Some(point(px(x as f32), px(y as f32)))
}

fn parse_pixel_size_env_var(value: &str) -> Option<Size<Pixels>> {
    let mut parts = value.split(',');
    let width: usize = parts.next()?.parse().ok()?;
    let height: usize = parts.next()?.parse().ok()?;
    Some(size(px(width as f32), px(height as f32)))
}

fn window_bounds_env_override() -> Option<Bounds<Pixels>> {
    ZED_WINDOW_POSITION
        .zip(*ZED_WINDOW_SIZE)
        .map(|(position, size)| Bounds {
            origin: position,
            size,
        })
}

fn project_window_title(project: &Project, cx: &App) -> String {
    let mut title = String::new();

    for (index, worktree) in project.visible_worktrees(cx).enumerate() {
        let name = worktree.read(cx).root_name_str();
        if index > 0 {
            title.push_str(", ");
        }
        title.push_str(name);
    }

    if title.is_empty() {
        // Keep the default untitled-window text instead of showing a blank title.
        "empty project".to_string()
    } else {
        title
    }
}

fn leader_border_for_pane(
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

struct DelayedDebouncedEditAction {
    task: Option<Task<()>>,
    cancel_channel: Option<oneshot::Sender<()>>,
}

impl DelayedDebouncedEditAction {
    fn new() -> DelayedDebouncedEditAction {
        DelayedDebouncedEditAction {
            task: None,
            cancel_channel: None,
        }
    }

    fn fire_new<F>(
        &mut self,
        delay: Duration,
        window: &mut Window,
        cx: &mut Context<Workspace>,
        func: F,
    ) where
        F: 'static
            + Send
            + FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) -> Task<Result<()>>,
    {
        if let Some(channel) = self.cancel_channel.take() {
            _ = channel.send(());
        }

        let (sender, mut receiver) = oneshot::channel::<()>();
        self.cancel_channel = Some(sender);

        let previous_task = self.task.take();
        self.task = Some(cx.spawn_in(window, async move |workspace, cx| {
            let mut timer = cx.background_executor().timer(delay).fuse();
            if let Some(previous_task) = previous_task {
                previous_task.await;
            }

            futures::select_biased! {
                _ = receiver => return,
                    _ = timer => {}
            }

            if let Some(result) = workspace
                .update_in(cx, |workspace, window, cx| (func)(workspace, window, cx))
                .log_err()
            {
                result.await.log_err();
            }
        }));
    }
}
