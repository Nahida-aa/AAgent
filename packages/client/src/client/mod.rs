mod auth;
mod cloud;
mod connect;
mod core;
mod llm;
mod reconnect;
mod rpc_impl;
mod subscribe;

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use parking_lot::RwLock;
use std::sync::Arc;

use cloud_api_client::CloudApiClient;
use gpui::{AsyncApp, Task};
use http_client::HttpClientWithUrl;
use parking_lot::Mutex;
use postage::watch;
use rpc::Peer;
use rpc::ProtoMessageHandlerSet;

use crate::credentials::{ClientCredentialsProvider, Credentials};
use crate::error::EstablishConnectionError;
use crate::status::Status;
use crate::subscription::MessageToClientHandler;
use crate::telemetry::Telemetry;

pub use crate::subscription::Subscription;

pub struct Client {
    pub(crate) id: AtomicU64,
    pub(crate) peer: Arc<Peer>,
    pub(crate) http: Arc<HttpClientWithUrl>,
    pub(crate) cloud_client: Arc<CloudApiClient>,
    pub(crate) telemetry: Arc<Telemetry>,
    pub(crate) credentials_provider: ClientCredentialsProvider,
    pub(crate) state: RwLock<ClientState>,
    pub(crate) handler_set: Mutex<ProtoMessageHandlerSet>,
    pub(crate) message_to_client_handlers: Mutex<Vec<MessageToClientHandler>>,
    pub(crate) sign_out_tx: Mutex<Option<futures::channel::mpsc::UnboundedSender<()>>>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) authenticate: RwLock<
        Option<Box<dyn 'static + Send + Sync + Fn(&AsyncApp) -> Task<anyhow::Result<Credentials>>>>,
    >,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) establish_connection: RwLock<
        Option<
            Box<
                dyn 'static
                    + Send
                    + Sync
                    + Fn(
                        &Credentials,
                        &AsyncApp,
                    ) -> Task<Result<Connection, EstablishConnectionError>>,
            >,
        >,
    >,

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) rpc_url: RwLock<Option<url::Url>>,
}

pub(crate) struct ClientState {
    pub(crate) credentials: Option<Credentials>,
    pub(crate) status: (watch::Sender<Status>, watch::Receiver<Status>),
    pub(crate) cloud_connection_id: (watch::Sender<u64>, watch::Receiver<u64>),
    pub(crate) _reconnect_task: Option<Task<()>>,
    pub(crate) _cloud_connection_task: Option<Task<()>>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            credentials: None,
            status: watch::channel_with(Status::SignedOut),
            cloud_connection_id: watch::channel_with(0),
            _reconnect_task: None,
            _cloud_connection_task: None,
        }
    }
}
