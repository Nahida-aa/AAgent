use std::sync::Arc;

use gpui::{App, TaskExt, actions};

use crate::client::Client;

actions!(
    client,
    [
        /// Signs in to Zed account.
        SignIn,
        /// Signs out of Zed account.
        SignOut,
        /// Reconnects to the collaboration server.
        Reconnect
    ]
);

pub fn init(client: &Arc<Client>, cx: &mut App) {
    let client = Arc::downgrade(client);
    cx.on_action({
        let client = client.clone();
        move |_: &SignIn, cx| {
            if let Some(client) = client.upgrade() {
                cx.spawn(async move |cx| client.sign_in_with_optional_connect(true, cx).await)
                    .detach_and_log_err(cx);
            }
        }
    })
    .on_action({
        let client = client.clone();
        move |_: &SignOut, cx| {
            if let Some(client) = client.upgrade() {
                cx.spawn(async move |cx| {
                    client.sign_out(cx).await;
                })
                .detach();
            }
        }
    })
    .on_action({
        let client = client;
        move |_: &Reconnect, cx| {
            if let Some(client) = client.upgrade() {
                cx.spawn(async move |cx| {
                    client.reconnect(cx);
                })
                .detach();
            }
        }
    });
}
