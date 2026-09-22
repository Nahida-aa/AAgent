use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::{FutureExt as _, Stream, StreamExt as _};
use gpui::AsyncApp;
use rpc::proto::{
    AnyTypedEnvelope, Envelope as ProtoEnvelope, EnvelopedMessage, RequestMessage, TypedEnvelope,
};
use rpc::{AnyProtoClient, Peer, ProtoClient, ProtoMessageHandlerSet};

use super::Client;

impl Client {
    pub fn send<T: EnvelopedMessage>(&self, message: T) -> anyhow::Result<()> {
        log::debug!("rpc send. client_id:{}, name:{}", self.id(), message_type);
        let connection_id = self.connection_id()?;
        self.peer.send_dynamic(connection_id, envelope)
    }

    pub fn request<T: RequestMessage>(
        &self,
        request: T,
    ) -> impl std::future::Future<Output = anyhow::Result<T::Response>> + use<T> {
        self.request_dynamic(envelope, request_type).boxed()
    }

    pub fn request_stream<T: RequestMessage>(
        &self,
        request: T,
    ) -> impl std::future::Future<Output = anyhow::Result<impl Stream<Item = anyhow::Result<T::Response>>>>
    {
        let client_id = self.id();
        let response = self.connection_id().map(|connection_id| {
            self.peer
                .request_stream_dynamic(connection_id, envelope, request_type)
        });

        async move {
            log::debug!(
                "rpc stream request start. client_id:{}. name:{}",
                client_id,
                request_type
            );
            let response = response?.await;
            log::debug!(
                "rpc stream request opened. client_id:{}. name:{}",
                client_id,
                request_type
            );
            response
        }
        .boxed()
    }

    pub fn request_envelope<T: RequestMessage>(
        &self,
        request: T,
    ) -> impl std::future::Future<Output = anyhow::Result<TypedEnvelope<T::Response>>> + use<T>
    {
        let client_id = self.id();
        log::debug!(
            "rpc request start. client_id:{}. name:{}",
            client_id,
            T::NAME
        );
        let response = self
            .connection_id()
            .map(|conn_id| self.peer.request_envelope(conn_id, request));
        async move {
            let response = response?.await;
            log::debug!(
                "rpc request finish. client_id:{}. name:{}",
                client_id,
                T::NAME
            );
            response
        }
    }

    pub fn request_dynamic(
        &self,
        envelope: rpc::proto::Envelope,
        request_type: &'static str,
    ) -> impl std::future::Future<Output = anyhow::Result<rpc::proto::Envelope>> + use<> {
        let client_id = self.id();
        log::debug!(
            "rpc request start. client_id:{}. name:{}",
            client_id,
            request_type
        );
        let response = self
            .connection_id()
            .map(|conn_id| self.peer.request_dynamic(conn_id, envelope, request_type));
        async move {
            let response = response?.await;
            log::debug!(
                "rpc request finish. client_id:{}. name:{}",
                client_id,
                request_type
            );
            Ok(response?.0)
        }
    }

    pub(crate) fn handle_message(
        self: &Arc<Self>,
        message: Box<dyn AnyTypedEnvelope>,
        cx: &AsyncApp,
    ) {
        let sender_id = message.sender_id();
        let request_id = message.message_id();
        let type_name = message.payload_type_name();
        let original_sender_id = message.original_sender_id();

        if let Some(future) = ProtoMessageHandlerSet::handle_message(
            &self.handler_set,
            message,
            self.clone().into(),
            cx.clone(),
        ) {
            let client_id = self.id();
            log::debug!(
                "rpc message received. client_id:{}, sender_id:{:?}, type:{}",
                client_id,
                original_sender_id,
                type_name
            );
            cx.spawn(async move |_| match future.await {
            Ok(()) => {
                log::debug!("rpc message handled. client_id:{client_id}, sender_id:{original_sender_id:?}, type:{type_name}");
            }
            Err(error) => {
                log::error!("error handling message. client_id:{client_id}, sender_id:{original_sender_id:?}, type:{type_name}, error:{error:#}");
            }
        })
        .detach();
        } else {
            log::info!("unhandled message {}", type_name);
            self.peer
                .respond_with_unhandled_message(sender_id.into(), request_id, type_name)
                .log_err();
        }
    }
}

impl ProtoClient for Client {
    fn request(
        &self,
        envelope: proto::Envelope,
        request_type: &'static str,
    ) -> BoxFuture<'static, Result<proto::Envelope>> {
        self.request_dynamic(envelope, request_type).boxed()
    }

    fn request_stream(
        &self,
        envelope: proto::Envelope,
        request_type: &'static str,
    ) -> BoxFuture<'static, Result<BoxStream<'static, Result<proto::Envelope>>>> {
        let client_id = self.id();
        let response = self.connection_id().map(|connection_id| {
            self.peer
                .request_stream_dynamic(connection_id, envelope, request_type)
        });

        async move {
            log::debug!(
                "rpc stream request start. client_id:{}. name:{}",
                client_id,
                request_type
            );
            let response = response?.await;
            log::debug!(
                "rpc stream request opened. client_id:{}. name:{}",
                client_id,
                request_type
            );
            response
        }
        .boxed()
    }

    fn send(&self, envelope: proto::Envelope, message_type: &'static str) -> Result<()> {
        log::debug!("rpc send. client_id:{}, name:{}", self.id(), message_type);
        let connection_id = self.connection_id()?;
        self.peer.send_dynamic(connection_id, envelope)
    }

    fn send_response(&self, envelope: proto::Envelope, message_type: &'static str) -> Result<()> {
        log::debug!(
            "rpc respond. client_id:{}, name:{}",
            self.id(),
            message_type
        );
        let connection_id = self.connection_id()?;
        self.peer.send_dynamic(connection_id, envelope)
    }

    fn message_handler_set(&self) -> &Mutex<ProtoMessageHandlerSet> { &self.handler_set }

    fn is_via_collab(&self) -> bool { true }

    fn has_wsl_interop(&self) -> bool { false }
}
