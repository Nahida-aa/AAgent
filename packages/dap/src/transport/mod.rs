//! Debug-adapter transport abstraction plus its concrete implementations
//! (TCP, stdio) and the shared [`TransportDelegate`] that drives them.

mod delegate;
#[cfg(any(test, feature = "test-support"))]
mod fake;
mod pending;
mod stdio;
mod tcp;
mod transport_trait;
mod types;

pub(crate) use delegate::TransportDelegate;
#[cfg(any(test, feature = "test-support"))]
pub use fake::{FakeTransport, FakeTransportKind, RequestHandling};
pub(crate) use pending::PendingRequests;
pub use stdio::StdioTransport;
pub use tcp::TcpTransport;
pub use transport_trait::Transport;
pub(crate) use types::{Command, IoHandler, IoKind, IoMessage, LogHandlers, LogKind};
