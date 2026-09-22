use std::cmp;
use std::sync::Arc;
use std::time::Duration;

use futures::{FutureExt as _, StreamExt as _};
use gpui::AsyncApp;
use rand::prelude::*;
use util::ResultExt as _;

use crate::constants::{INITIAL_RECONNECTION_DELAY, MAX_RECONNECTION_DELAY};

use super::Client;

impl Client {
    pub(crate) fn connect_to_cloud(self: &Arc<Self>, cx: &AsyncApp) {
        let this = self.clone();
        let task = cx.spawn(async move |cx| {
            #[cfg(any(test, feature = "test-support"))]
            let mut rng = StdRng::seed_from_u64(0);
            #[cfg(not(any(test, feature = "test-support")))]
            let mut rng = StdRng::from_os_rng();

            let mut delay = INITIAL_RECONNECTION_DELAY;
            loop {
                match Self::run_cloud_connection(&this, cx).await {
                    Ok(()) => {
                        log::info!("cloud websocket disconnected, will reconnect");
                        delay = INITIAL_RECONNECTION_DELAY;
                    }
                    Err(err) => {
                        log::warn!(
                            "cloud websocket connect failed: {err:#}; retrying in {delay:?}"
                        );
                    }
                }

                let jitter = Duration::from_millis(rng.random_range(0..delay.as_millis() as u64));
                cx.background_executor().timer(delay + jitter).await;
                delay = cmp::min(delay * 2, MAX_RECONNECTION_DELAY);
            }
        });
        self.state.write()._cloud_connection_task = Some(task);
    }

    async fn run_cloud_connection(self: &Arc<Self>, cx: &mut AsyncApp) -> Result<()> {
        let connect_task = cx.update({
            let cloud_client = self.cloud_client.clone();
            move |cx| cloud_client.connect(cx)
        })?;
        let connection = connect_task.await?;

        let (mut messages, _cloud_io_task) = cx.update(|cx| connection.spawn(cx));

        {
            let mut state = self.state.write();
            let mut cloud_connection_id = state.cloud_connection_id.0.borrow_mut();
            *cloud_connection_id = cloud_connection_id.saturating_add(1);
        }

        while let Some(message) = messages.next().await {
            if let Some(message) = message.log_err() {
                self.handle_message_to_client(message, cx);
            }
        }

        Ok(())
    }
}
