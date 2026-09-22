use anyhow::{Context as _, Result};
use futures::{AsyncRead, AsyncWrite};
use gpui::{AsyncApp, Task};

use crate::adapters::{DebugAdapterBinary, TcpArguments};

use super::stdio::StdioTransport;
use super::tcp::TcpTransport;
use super::types::LogHandlers;

pub trait Transport: Send + Sync {
    fn has_adapter_logs(&self) -> bool;
    fn tcp_arguments(&self) -> Option<TcpArguments>;
    fn connect(
        &mut self,
    ) -> Task<
        Result<(
            Box<dyn AsyncWrite + Unpin + Send + 'static>,
            Box<dyn AsyncRead + Unpin + Send + 'static>,
        )>,
    >;
    fn kill(&mut self);

    #[cfg(any(test, feature = "test-support"))]
    fn as_fake(&self) -> &super::fake::FakeTransport { unreachable!() }
}

/// Builds the appropriate [`Transport`] for `binary`.
///
/// Test builds can inject either a fake stdio or fake TCP transport via
/// `binary.connection` / the fake registry.
pub(crate) async fn start(
    binary: &DebugAdapterBinary,
    log_handlers: LogHandlers,
    cx: &mut AsyncApp,
) -> Result<Box<dyn Transport>> {
    #[cfg(any(test, feature = "test-support"))]
    if cfg!(any(test, feature = "test-support")) {
        use super::fake::FakeTransport;
        if let Some(connection) = binary.connection.clone() {
            return Ok(Box::new(FakeTransport::start_tcp(connection, cx).await?));
        } else {
            return Ok(Box::new(FakeTransport::start_stdio(cx).await?));
        }
    }

    if binary.connection.is_some() {
        Ok(Box::new(
            TcpTransport::start(binary, log_handlers, cx).await?,
        ))
    } else {
        Ok(Box::new(
            StdioTransport::start(binary, log_handlers, cx).await?,
        ))
    }
}
