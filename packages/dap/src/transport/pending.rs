use anyhow::{Context as _, Result, anyhow, bail};
#[cfg(any(test, feature = "test-support"))]
use async_pipe::{PipeReader, PipeWriter};
use dap_types::{
    ErrorResponse,
    messages::{Message, Response},
};
use futures::{AsyncRead, AsyncReadExt as _, AsyncWrite, FutureExt as _, channel::oneshot, select};
use gpui::{AppContext as _, AsyncApp, BackgroundExecutor, Task};
use parking_lot::Mutex;
use proto::ErrorExt;
use settings::Settings as _;
use smallvec::SmallVec;
use smol::{
    channel::{Receiver, Sender, unbounded},
    io::{AsyncBufReadExt as _, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use task::TcpArgumentsTemplate;
use util::{ConnectionResult, ResultExt, process::Child};

use crate::{
    adapters::{DebugAdapterBinary, TcpArguments},
    client::DapMessageHandler,
    debugger_settings::DebuggerSettings,
};

pub(crate) struct PendingRequests {
    inner: Option<HashMap<u64, oneshot::Sender<Result<Response>>>>,
}

impl PendingRequests {
    pub(crate) fn new() -> Self {
        Self {
            inner: Some(HashMap::default()),
        }
    }

    pub(crate) fn flush(&mut self, e: anyhow::Error) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        for (_, sender) in inner.drain() {
            sender.send(Err(e.cloned())).ok();
        }
    }

    pub(crate) fn insert(
        &mut self,
        sequence_id: u64,
        callback_tx: oneshot::Sender<Result<Response>>,
    ) -> Result<()> {
        let Some(inner) = self.inner.as_mut() else {
            bail!("client is closed")
        };
        inner.insert(sequence_id, callback_tx);
        Ok(())
    }

    pub(crate) fn remove(
        &mut self,
        sequence_id: u64,
    ) -> Result<Option<oneshot::Sender<Result<Response>>>> {
        let Some(inner) = self.inner.as_mut() else {
            bail!("client is closed");
        };
        Ok(inner.remove(&sequence_id))
    }

    pub(crate) fn shutdown(&mut self) {
        self.flush(anyhow::anyhow!("transport shutdown"));
        self.inner = None;
    }
}
