#![cfg(any(test, feature = "test-support"))]

use anyhow::{Context as _, Result, anyhow};
use async_pipe::{PipeReader, PipeWriter};
use dap_types::{
    ErrorResponse,
    messages::{Message, Response},
};
use futures::{AsyncRead, AsyncWrite};
use gpui::{AppContext as _, AsyncApp, BackgroundExecutor, Task};
use parking_lot::Mutex;
use proto::ErrorExt;
use smol::{
    io::{AsyncBufReadExt as _, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::adapters::TcpArguments;
use util::{ConnectionResult, ResultExt};

use super::delegate::TransportDelegate;
use super::transport_trait::Transport;
use super::types::RequestHandling;

type RequestHandler = Box<dyn Send + FnMut(u64, serde_json::Value) -> RequestHandling<Response>>;
type ResponseHandler = Box<dyn Send + Fn(Response)>;

pub struct FakeTransport {
    // for sending fake response back from adapter side
    request_handlers: Arc<Mutex<HashMap<&'static str, RequestHandler>>>,
    // for reverse request responses
    response_handlers: Arc<Mutex<HashMap<&'static str, ResponseHandler>>>,
    message_handler: Option<Task<Result<()>>>,
    kind: FakeTransportKind,
}

pub enum FakeTransportKind {
    Stdio {
        stdin_writer: Option<PipeWriter>,
        stdout_reader: Option<PipeReader>,
    },
    Tcp {
        connection: TcpArguments,
        executor: BackgroundExecutor,
    },
}

#[cfg(any(test, feature = "test-support"))]
impl FakeTransport {
    pub fn on_request<R: dap_types::requests::Request, F>(&self, mut handler: F)
    where
        F: 'static
            + Send
            + FnMut(u64, R::Arguments) -> RequestHandling<Result<R::Response, ErrorResponse>>,
    {
        self.request_handlers.lock().insert(
            R::COMMAND,
            Box::new(move |seq, args| {
                let result = handler(seq, serde_json::from_value(args).unwrap());
                let RequestHandling::Respond(response) = result else {
                    return RequestHandling::Exit;
                };
                let response = match response {
                    Ok(response) => Response {
                        seq: seq + 1,
                        request_seq: seq,
                        success: true,
                        command: R::COMMAND.into(),
                        body: Some(serde_json::to_value(response).unwrap()),
                        message: None,
                    },
                    Err(response) => Response {
                        seq: seq + 1,
                        request_seq: seq,
                        success: false,
                        command: R::COMMAND.into(),
                        body: Some(serde_json::to_value(response).unwrap()),
                        message: None,
                    },
                };
                RequestHandling::Respond(response)
            }),
        );
    }

    pub fn on_response<R: dap_types::requests::Request, F>(&self, handler: F)
    where
        F: 'static + Send + Fn(Response),
    {
        self.response_handlers
            .lock()
            .insert(R::COMMAND, Box::new(handler));
    }

    pub(crate) async fn start_tcp(connection: TcpArguments, cx: &mut AsyncApp) -> Result<Self> {
        Ok(Self {
            request_handlers: Arc::new(Mutex::new(HashMap::default())),
            response_handlers: Arc::new(Mutex::new(HashMap::default())),
            message_handler: None,
            kind: FakeTransportKind::Tcp {
                connection,
                executor: cx.background_executor().clone(),
            },
        })
    }

    pub(crate) async fn handle_messages(
        request_handlers: Arc<Mutex<HashMap<&'static str, RequestHandler>>>,
        response_handlers: Arc<Mutex<HashMap<&'static str, ResponseHandler>>>,
        stdin_reader: PipeReader,
        stdout_writer: PipeWriter,
    ) -> Result<()> {
        use dap_types::requests::{Request, RunInTerminal, StartDebugging};
        use serde_json::json;

        let mut reader = BufReader::new(stdin_reader);
        let stdout_writer = Arc::new(smol::lock::Mutex::new(stdout_writer));
        let mut buffer = String::new();

        loop {
            match TransportDelegate::receive_server_message(&mut reader, &mut buffer, None).await {
                ConnectionResult::Timeout => {
                    anyhow::bail!("Timed out when connecting to debugger");
                }
                ConnectionResult::ConnectionReset => {
                    log::info!("Debugger closed the connection");
                    break Ok(());
                }
                ConnectionResult::Result(Err(e)) => break Err(e),
                ConnectionResult::Result(Ok(message)) => {
                    match message {
                        Message::Request(request) => {
                            // redirect reverse requests to stdout writer/reader
                            if request.command == RunInTerminal::COMMAND
                                || request.command == StartDebugging::COMMAND
                            {
                                let message =
                                    serde_json::to_string(&Message::Request(request)).unwrap();

                                let mut writer = stdout_writer.lock().await;
                                writer
                                    .write_all(
                                        TransportDelegate::build_rpc_message(message).as_bytes(),
                                    )
                                    .await
                                    .unwrap();
                                writer.flush().await.unwrap();
                            } else {
                                let response = if let Some(handle) =
                                    request_handlers.lock().get_mut(request.command.as_str())
                                {
                                    handle(request.seq, request.arguments.unwrap_or(json!({})))
                                } else {
                                    panic!("No request handler for {}", request.command);
                                };
                                let response = match response {
                                    RequestHandling::Respond(response) => response,
                                    RequestHandling::Exit => {
                                        break Err(anyhow!("exit in response to request"));
                                    }
                                };
                                let success = response.success;
                                let message =
                                    serde_json::to_string(&Message::Response(response)).unwrap();

                                let mut writer = stdout_writer.lock().await;
                                writer
                                    .write_all(
                                        TransportDelegate::build_rpc_message(message).as_bytes(),
                                    )
                                    .await
                                    .unwrap();

                                if request.command == dap_types::requests::Initialize::COMMAND
                                    && success
                                {
                                    let message = serde_json::to_string(&Message::Event(Box::new(
                                        dap_types::messages::Events::Initialized(Some(
                                            Default::default(),
                                        )),
                                    )))
                                    .unwrap();
                                    writer
                                        .write_all(
                                            TransportDelegate::build_rpc_message(message)
                                                .as_bytes(),
                                        )
                                        .await
                                        .unwrap();
                                }

                                writer.flush().await.unwrap();
                            }
                        }
                        Message::Event(event) => {
                            let message = serde_json::to_string(&Message::Event(event)).unwrap();

                            let mut writer = stdout_writer.lock().await;
                            writer
                                .write_all(TransportDelegate::build_rpc_message(message).as_bytes())
                                .await
                                .unwrap();
                            writer.flush().await.unwrap();
                        }
                        Message::Response(response) => {
                            if let Some(handle) =
                                response_handlers.lock().get(response.command.as_str())
                            {
                                handle(response);
                            } else {
                                log::error!("No response handler for {}", response.command);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn start_stdio(cx: &mut AsyncApp) -> Result<Self> {
        let (stdin_writer, stdin_reader) = async_pipe::pipe();
        let (stdout_writer, stdout_reader) = async_pipe::pipe();
        let kind = FakeTransportKind::Stdio {
            stdin_writer: Some(stdin_writer),
            stdout_reader: Some(stdout_reader),
        };

        let mut this = Self {
            request_handlers: Arc::new(Mutex::new(HashMap::default())),
            response_handlers: Arc::new(Mutex::new(HashMap::default())),
            message_handler: None,
            kind,
        };

        let request_handlers = this.request_handlers.clone();
        let response_handlers = this.response_handlers.clone();

        this.message_handler = Some(cx.background_spawn(Self::handle_messages(
            request_handlers,
            response_handlers,
            stdin_reader,
            stdout_writer,
        )));

        Ok(this)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Transport for FakeTransport {
    pub(crate) fn tcp_arguments(&self) -> Option<TcpArguments> {
        match &self.kind {
            FakeTransportKind::Stdio { .. } => None,
            FakeTransportKind::Tcp { connection, .. } => Some(connection.clone()),
        }
    }

    pub(crate) fn connect(
        &mut self,
    ) -> Task<
        Result<(
            Box<dyn AsyncWrite + Unpin + Send + 'static>,
            Box<dyn AsyncRead + Unpin + Send + 'static>,
        )>,
    > {
        let result = match &mut self.kind {
            FakeTransportKind::Stdio {
                stdin_writer,
                stdout_reader,
            } => util::maybe!({
                Ok((
                    Box::new(stdin_writer.take().context("Cannot reconnect")?) as _,
                    Box::new(stdout_reader.take().context("Cannot reconnect")?) as _,
                ))
            }),
            FakeTransportKind::Tcp { executor, .. } => {
                let (stdin_writer, stdin_reader) = async_pipe::pipe();
                let (stdout_writer, stdout_reader) = async_pipe::pipe();

                let request_handlers = self.request_handlers.clone();
                let response_handlers = self.response_handlers.clone();

                self.message_handler = Some(executor.spawn(Self::handle_messages(
                    request_handlers,
                    response_handlers,
                    stdin_reader,
                    stdout_writer,
                )));

                Ok((Box::new(stdin_writer) as _, Box::new(stdout_reader) as _))
            }
        };
        Task::ready(result)
    }

    pub(crate) fn has_adapter_logs(&self) -> bool { false }

    pub(crate) fn kill(&mut self) { self.message_handler.take(); }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn as_fake(&self) -> &FakeTransport { self }
}
