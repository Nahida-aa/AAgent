use anyhow::{Context as _, Result};
use gpui::{Context, Window};

use super::MultiWorkspace;
use crate::{CloseIntent, CloseWindow, Workspace};

impl MultiWorkspace {
    pub fn close_window(&mut self, _: &CloseWindow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(window_handle) = window.window_handle().downcast::<Self>() else {
            log::error!("cannot close a window whose root is not a MultiWorkspace");
            return;
        };
        cx.spawn(async move |_, cx| {
            if !crate::prepare_window_to_close(window_handle, CloseIntent::CloseWindow, cx)
                .await
                .context("preparing the window to close")?
            {
                return anyhow::Ok(());
            }

            crate::flush_windows_serialization(&[window_handle], cx).await;

            window_handle
                .update(cx, |_, window, _cx| {
                    window.remove_window();
                })
                .context("removing the closed window")?;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}
