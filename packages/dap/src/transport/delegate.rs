use anyhow::{Context as _, Result, anyhow};
use dap_types::{
    ErrorResponse,
    messages::{Message, Response},
};
use futures::{AsyncRead, AsyncReadExt as _, AsyncWrite, FutureExt as _, channel::oneshot, select};
use gpui::{AppContext as _, AsyncApp, BackgroundExecutor, Task};
use parking_lot::Mutex;
use settings::Settings as _;
use smol::{
    channel::{Receiver, Sender, unbounded},
    io::{AsyncBufReadExt as _, AsyncWriteExt, BufReader},
};
use std::sync::Arc;

use crate::adapters::{DebugAdapterBinary, TcpArguments};
use crate::client::DapMessageHandler;
use crate::debugger_settings::DebuggerSettings;
use util::{ConnectionResult, ResultExt};

use super::pending::PendingRequests;
use super::transport_trait::{Transport, start};
use super::types::{Command, IoHandler, IoKind, IoMessage, LogHandlers, LogKind};

pub(crate) struct TransportDelegate {
    log_handlers: LogHandlers,
    pub(crate) pending_requests: Arc<Mutex<PendingRequests>>,
    pub(crate) transport: Mutex<Box<dyn Transport>>,
    pub(crate) server_tx: smol::lock::Mutex<Option<Sender<Message>>>,
    tasks: Mutex<Vec<Task<()>>>,
}

impl TransportDelegate {
    pub(crate) async fn start(binary: &DebugAdapterBinary, cx: &mut AsyncApp) -> Result<Self> {
        let log_handlers: LogHandlers = Default::default();
        let transport = start(binary, log_handlers.clone(), cx).await?;
        Ok(Self {
            transport: Mutex::new(transport),
            log_handlers,
            server_tx: Default::default(),
            pending_requests: Arc::new(Mutex::new(PendingRequests::new())),
            tasks: Default::default(),
        })
    }

    pub async fn connect(
        &self,
        message_handler: DapMessageHandler,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        let (server_tx, client_rx) = unbounded::<Message>();
        self.tasks.lock().clear();

        let log_dap_communications =
            cx.update(|cx| DebuggerSettings::get_global(cx).log_dap_communications);

        let connect = self.transport.lock().connect();
        let (input, output) = connect.await?;

        let log_handler = if log_dap_communications {
            Some(self.log_handlers.clone())
        } else {
            None
        };

        let pending_requests = self.pending_requests.clone();
        let output_log_handler = log_handler.clone();
        {
            let mut tasks = self.tasks.lock();
            tasks.push(cx.background_spawn(async move {
                match Self::recv_from_server(
                    output,
                    message_handler,
                    pending_requests.clone(),
                    output_log_handler,
                )
                .await
                {
                    Ok(()) => {
                        pending_requests
                            .lock()
                            .flush(anyhow!("debugger shutdown unexpectedly"));
                    }
                    Err(e) => {
                        pending_requests.lock().flush(e);
                    }
                }
            }));

            tasks.push(cx.background_spawn(async move {
                match Self::send_to_server(input, client_rx, log_handler).await {
                    Ok(()) => {}
                    Err(e) => log::error!("Error handling debugger input: {e}"),
                }
            }));
        }

        *self.server_tx.lock().await = Some(server_tx.clone());

        Ok(())
    }

    pub(crate) fn tcp_arguments(&self) -> Option<TcpArguments> {
        self.transport.lock().tcp_arguments()
    }

    pub(crate) async fn send_message(&self, message: Message) -> Result<()> {
        if let Some(server_tx) = self.server_tx.lock().await.as_ref() {
            server_tx.send(message).await.context("sending message")
        } else {
            anyhow::bail!("Server tx already dropped")
        }
    }

    pub(crate) async fn handle_adapter_log(
        stdout: impl AsyncRead + Unpin + Send + 'static,
        iokind: IoKind,
        log_handlers: LogHandlers,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        loop {
            line.clear();

            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    log::debug!("handle_adapter_log: {}", e);
                    break;
                }
            }

            // Clean up logs by trimming unnecessary whitespace/newlines before inserting into log.
            let line = line.trim();

            log::debug!("stderr: {line}");

            for (kind, handler) in log_handlers.lock().iter_mut() {
                if matches!(kind, LogKind::Adapter) {
                    handler(iokind, None, line);
                }
            }
        }
    }

    pub(crate) fn build_rpc_message(message: String) -> String {
        format!("Content-Length: {}\r\n\r\n{}", message.len(), message)
    }

    pub(crate) async fn send_to_server<Stdin>(
        mut server_stdin: Stdin,
        client_rx: Receiver<Message>,
        log_handlers: Option<LogHandlers>,
    ) -> Result<()>
    where
        Stdin: AsyncWrite + Unpin + Send + 'static,
    {
        let result = loop {
            match client_rx.recv().await {
                Ok(message) => {
                    let command = match &message {
                        Message::Request(request) => Some(request.command.as_str()),
                        Message::Response(response) => Some(response.command.as_str()),
                        _ => None,
                    };

                    let message = match serde_json::to_string(&message) {
                        Ok(message) => message,
                        Err(e) => break Err(e.into()),
                    };

                    if let Some(log_handlers) = log_handlers.as_ref() {
                        for (kind, log_handler) in log_handlers.lock().iter_mut() {
                            if matches!(kind, LogKind::Rpc) {
                                log_handler(IoKind::StdIn, command, &message);
                            }
                        }
                    }

                    if let Err(e) = server_stdin
                        .write_all(Self::build_rpc_message(message).as_bytes())
                        .await
                    {
                        break Err(e.into());
                    }

                    if let Err(e) = server_stdin.flush().await {
                        break Err(e.into());
                    }
                }
                Err(error) => break Err(error.into()),
            }
        };

        log::debug!("Handle adapter input dropped");

        result
    }

    pub(crate) async fn recv_from_server<Stdout>(
        server_stdout: Stdout,
        mut message_handler: DapMessageHandler,
        pending_requests: Arc<Mutex<PendingRequests>>,
        log_handlers: Option<LogHandlers>,
    ) -> Result<()>
    where
        Stdout: AsyncRead + Unpin + Send + 'static,
    {
        let mut recv_buffer = String::new();
        let mut reader = BufReader::new(server_stdout);

        let result = loop {
            let result =
                Self::receive_server_message(&mut reader, &mut recv_buffer, log_handlers.as_ref())
                    .await;
            match result {
                ConnectionResult::Timeout => anyhow::bail!("Timed out when connecting to debugger"),
                ConnectionResult::ConnectionReset => {
                    log::info!("Debugger closed the connection");
                    return Ok(());
                }
                ConnectionResult::Result(Ok(Message::Response(res))) => {
                    let tx = pending_requests.lock().remove(res.request_seq)?;
                    if let Some(tx) = tx {
                        if let Err(e) = tx.send(Self::process_response(res)) {
                            log::trace!("Did not send response `{:?}` for a cancelled", e);
                        }
                    } else {
                        message_handler(Message::Response(res))
                    }
                }
                ConnectionResult::Result(Ok(message)) => message_handler(message),
                ConnectionResult::Result(Err(e)) => break Err(e),
            }
        };

        log::debug!("Handle adapter output dropped");

        result
    }

    pub(crate) fn process_response(response: Response) -> Result<Response> {
        if response.success {
            Ok(response)
        } else {
            if let Some(error_message) = response
                .body
                .clone()
                .and_then(|body| serde_json::from_value::<ErrorResponse>(body).ok())
                .and_then(|response| response.error.map(|msg| msg.format))
                .or_else(|| response.message.clone())
            {
                anyhow::bail!(error_message);
            };

            anyhow::bail!(
                "Received error response from adapter. Response: {:?}",
                response
            );
        }
    }

    pub(crate) async fn receive_server_message<Stdout>(
        reader: &mut BufReader<Stdout>,
        buffer: &mut String,
        log_handlers: Option<&LogHandlers>,
    ) -> ConnectionResult<Message>
    where
        Stdout: AsyncRead + Unpin + Send + 'static,
    {
        let mut content_length = None;
        loop {
            buffer.clear();
            match reader.read_line(buffer).await {
                Ok(0) => return ConnectionResult::ConnectionReset,
                Ok(_) => {}
                Err(e) => return ConnectionResult::Result(Err(e.into())),
            };

            if buffer == "\r\n" {
                break;
            }

            if let Some(("Content-Length", value)) = buffer.trim().split_once(": ") {
                match value.parse().context("invalid content length") {
                    Ok(length) => content_length = Some(length),
                    Err(e) => return ConnectionResult::Result(Err(e)),
                }
            }
        }

        let content_length = match content_length.context("missing content length") {
            Ok(length) => length,
            Err(e) => return ConnectionResult::Result(Err(e)),
        };

        let mut content = vec![0; content_length];
        if let Err(e) = reader
            .read_exact(&mut content)
            .await
            .with_context(|| "reading after a loop")
        {
            return ConnectionResult::Result(Err(e));
        }

        let message_str = match std::str::from_utf8(&content).context("invalid utf8 from server") {
            Ok(str) => str,
            Err(e) => return ConnectionResult::Result(Err(e)),
        };

        let message =
            serde_json::from_str::<Message>(message_str).context("deserializing server message");

        if let Some(log_handlers) = log_handlers {
            let command = match &message {
                Ok(Message::Request(request)) => Some(request.command.as_str()),
                Ok(Message::Response(response)) => Some(response.command.as_str()),
                _ => None,
            };

            for (kind, log_handler) in log_handlers.lock().iter_mut() {
                if matches!(kind, LogKind::Rpc) {
                    log_handler(IoKind::StdOut, command, message_str);
                }
            }
        }

        ConnectionResult::Result(message)
    }

    pub fn has_adapter_logs(&self) -> bool { self.transport.lock().has_adapter_logs() }

    pub fn add_log_handler<F>(&self, f: F, kind: LogKind)
    where
        F: 'static + Send + FnMut(IoKind, Option<&Command>, &IoMessage),
    {
        let mut log_handlers = self.log_handlers.lock();
        log_handlers.push((kind, Box::new(f)));
    }
}
