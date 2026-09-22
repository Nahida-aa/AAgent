use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};

use gpui::{App, AsyncApp, Entity, WeakEntity};
use rpc::proto::{EnvelopedMessage, TypedEnvelope};
use rpc::{AnyProtoClient, EntityMessageSubscriber, ProtoMessageHandlerSet};

use crate::client::Client;

pub type MessageToClientHandler = Box<
    dyn Fn(&cloud_api_client::websocket_protocol::MessageToClient, &mut App)
        + Send
        + Sync
        + 'static,
>;

pub struct GlobalClient(pub(crate) Arc<Client>);

impl gpui::Global for GlobalClient {}

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
        // 原样搬入
    }
}

pub struct PendingEntitySubscription<T: 'static> {
    pub(crate) client: Arc<Client>,
    pub(crate) remote_id: u64,
    pub(crate) _entity_type: PhantomData<T>,
    pub(crate) consumed: bool,
}

impl<T: 'static> PendingEntitySubscription<T> {
    pub fn set_entity(mut self, entity: &Entity<T>, cx: &AsyncApp) -> Subscription {
        self.consumed = true;
        let mut handlers = self.client.handler_set.lock();
        let id = (TypeId::of::<T>(), self.remote_id);
        let Some(EntityMessageSubscriber::Pending(messages)) =
            handlers.entities_by_type_and_remote_id.remove(&id)
        else {
            unreachable!()
        };

        handlers.entities_by_type_and_remote_id.insert(
            id,
            EntityMessageSubscriber::Entity {
                handle: entity.downgrade().into(),
            },
        );
        drop(handlers);
        for message in messages {
            let client_id = self.client.id();
            let type_name = message.payload_type_name();
            let sender_id = message.original_sender_id();
            log::debug!(
                "handling queued rpc message. client_id:{}, sender_id:{:?}, type:{}",
                client_id,
                sender_id,
                type_name
            );
            self.client.handle_message(message, cx);
        }
        Subscription::Entity {
            client: Arc::downgrade(&self.client),
            id,
        }
    }
}

impl<T: 'static> Drop for PendingEntitySubscription<T> {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self.client.handler_set.lock();
            if let Some(EntityMessageSubscriber::Pending(messages)) = state
                .entities_by_type_and_remote_id
                .remove(&(TypeId::of::<T>(), self.remote_id))
            {
                for message in messages {
                    log::info!("unhandled message {}", message.payload_type_name());
                }
            }
        }
    }
}
