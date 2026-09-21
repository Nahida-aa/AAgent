use std::{
    any::TypeId,
    sync::{Arc, atomic::AtomicU64},
};

pub struct Client {
    id: AtomicU64,
    peer: Arc<Peer>,
    http: Arc<HttpClientWithUrl>,
    cloud_client: Arc<CloudApiClient>,
    telemetry: Arc<Telemetry>,
    credentials_provider: ClientCredentialsProvider,
    state: RwLock<ClientState>,
    handler_set: Mutex<ProtoMessageHandlerSet>,
    message_to_client_handlers: Mutex<Vec<MessageToClientHandler>>,
    sign_out_tx: Mutex<Option<mpsc::UnboundedSender<()>>>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    authenticate:
        RwLock<Option<Box<dyn 'static + Send + Sync + Fn(&AsyncApp) -> Task<Result<Credentials>>>>>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    establish_connection: RwLock<
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
    rpc_url: RwLock<Option<Url>>,
}

pub enum Subscription {
    Entity {
        client: Weak<Client>,
        id: (TypeId, u64),
    },
    Message {
        client: Weak<Client>,
        id: TypeId,
    },
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self {
            Subscription::Entity { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.handler_set.lock();
                    let _ = state.entities_by_type_and_remote_id.remove(id);
                }
            }
            Subscription::Message { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.handler_set.lock();
                    let _ = state.entity_types_by_message_type.remove(id);
                    let _ = state.message_handlers.remove(id);
                }
            }
        }
    }
}
