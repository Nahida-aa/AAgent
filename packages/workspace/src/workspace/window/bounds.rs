use super::*;
static ZED_WINDOW_SIZE: LazyLock<Option<Size<Pixels>>> = LazyLock::new(|| {
    env::var("ZED_WINDOW_SIZE")
        .ok()
        .as_deref()
        .and_then(parse_pixel_size_env_var)
});

static ZED_WINDOW_POSITION: LazyLock<Option<Point<Pixels>>> = LazyLock::new(|| {
    env::var("ZED_WINDOW_POSITION")
        .ok()
        .as_deref()
        .and_then(parse_pixel_position_env_var)
});

impl Workspace {
    // │   ├── save_window_bounds
   pub(crate) fn save_window_bounds(&self, window: &mut Window, cx: &mut App) -> Task<()> {
        let Some(display) = window.display(cx) else {
            return Task::ready(());
        };
        let Ok(display_uuid) = display.uuid() else {
            return Task::ready(());
        };

        let window_bounds = window.inner_window_bounds();
        let database_id = self.database_id;
        let has_paths = !self.root_paths(cx).is_empty();
        let db = WorkspaceDb::global(cx);
        let kvp = db::kvp::KeyValueStore::global(cx);
        let native_window_state = if database_id.is_some() {
            window.native_window_state()
        } else {
            None
        };

        cx.background_executor().spawn(async move {
            if !has_paths {
                persistence::write_default_window_bounds(&kvp, window_bounds, display_uuid)
                    .await
                    .log_err();
            }
            if let Some(database_id) = database_id {
                db.set_window_open_status(
                    database_id,
                    SerializedWindowBounds(window_bounds),
                    display_uuid,
                    native_window_state,
                )
                .await
                .log_err();
            } else {
                persistence::write_default_window_bounds(&kvp, window_bounds, display_uuid)
                    .await
                    .log_err();
            }
        })
    }
    // │   ├── restore_native_window_state
    // │   ├── parse_pixel_position_env_var
    // │   └── parse_pixel_size_env_var
}

// │   ├── window_bounds_env_override
pub(crate) fn window_bounds_env_override() -> Option<Bounds<Pixels>> {
    ZED_WINDOW_POSITION
        .zip(*ZED_WINDOW_SIZE)
        .map(|(position, size)| Bounds {
            origin: position,
            size,
        })
}

pub(crate) fn restore_native_window_state(
    window_handle: WindowHandle<MultiWorkspace>,
    workspace_id: WorkspaceId,
    cx: &mut AsyncApp,
) {
    if window_bounds_env_override().is_some() {
        return;
    }
    let Some((Some(display), Some(native_window_state))) = cx
        .update(|cx| WorkspaceDb::global(cx))
        .native_window_state(workspace_id)
        .log_err()
        .flatten()
    else {
        return;
    };
    let display_connected = cx.update(|cx| {
        cx.displays()
            .into_iter()
            .any(|connected_display| connected_display.uuid().ok() == Some(display))
    });
    if !display_connected {
        return;
    }
    window_handle
        .update(cx, |_, window, _cx| {
            window.restore_native_window_state(&native_window_state);
        })
        .log_err();
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
