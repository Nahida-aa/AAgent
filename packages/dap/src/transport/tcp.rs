use anyhow::{Context as _, Result};
use futures::{AsyncRead, AsyncWrite, FutureExt as _, select};
use gpui::{AppContext as _, AsyncApp, BackgroundExecutor, Task};
use parking_lot::Mutex;
use settings::Settings as _;
use smol::net::{TcpListener, TcpStream};
use std::net::{IpAddr, SocketAddr};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use task::TcpArgumentsTemplate;
use util::{ResultExt, process::Child};

use crate::adapters::{DebugAdapterBinary, TcpArguments};
use crate::debugger_settings::DebuggerSettings;

use super::delegate::TransportDelegate;
use super::transport_trait::Transport;
use super::types::{IoKind, LogHandlers};

pub struct TcpTransport {
    executor: BackgroundExecutor,
    pub port: u16,
    pub host: IpAddr,
    pub timeout: u64,
    process: Arc<Mutex<Option<Child>>>,
    _stderr_task: Option<Task<()>>,
    _stdout_task: Option<Task<()>>,
}

impl TcpTransport {
    /// Get an open port to use with the tcp client when not supplied by debug config
    pub async fn port(host: &TcpArgumentsTemplate) -> Result<u16> {
        if let Some(port) = host.port {
            Ok(port)
        } else {
            Self::unused_port(host.host()).await
        }
    }

    pub async fn unused_port(host: IpAddr) -> Result<u16> {
        Ok(TcpListener::bind(SocketAddr::new(host, 0))
            .await?
            .local_addr()?
            .port())
    }

    pub(crate) async fn start(
        binary: &DebugAdapterBinary,
        log_handlers: LogHandlers,
        cx: &mut AsyncApp,
    ) -> Result<Self> {
        let connection_args = binary
            .connection
            .as_ref()
            .context("No connection arguments provided")?;

        let host = connection_args.host;
        let port = connection_args.port;

        let mut process = None;
        let mut stdout_task = None;
        let mut stderr_task = None;

        if let Some(command) = &binary.command {
            let mut command = util::command::new_std_command(&command);

            if let Some(cwd) = &binary.cwd {
                command.current_dir(cwd);
            }

            command.args(&binary.arguments);
            command.envs(&binary.envs);

            let mut p = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::piped())
                .with_context(|| "failed to start debug adapter.")?;

            stdout_task = p.stdout.take().map(|stdout| {
                cx.background_executor()
                    .spawn(TransportDelegate::handle_adapter_log(
                        stdout,
                        IoKind::StdOut,
                        log_handlers.clone(),
                    ))
            });
            stderr_task = p.stderr.take().map(|stderr| {
                cx.background_executor()
                    .spawn(TransportDelegate::handle_adapter_log(
                        stderr,
                        IoKind::StdErr,
                        log_handlers,
                    ))
            });
            process = Some(p);
        };

        let timeout = connection_args
            .timeout
            .unwrap_or_else(|| cx.update(|cx| DebuggerSettings::get_global(cx).timeout));

        log::info!(
            "Debug adapter has connected to TCP server {}:{}",
            host,
            port
        );

        let this = Self {
            executor: cx.background_executor().clone(),
            port,
            host,
            process: Arc::new(Mutex::new(process)),
            timeout,
            _stdout_task: stdout_task,
            _stderr_task: stderr_task,
        };

        Ok(this)
    }
}

impl Transport for TcpTransport {
    pub(crate) fn has_adapter_logs(&self) -> bool { true }

    pub(crate) fn kill(&mut self) {
        if let Some(process) = &mut *self.process.lock() {
            process.kill().log_err();
        }
    }

    pub(crate) fn tcp_arguments(&self) -> Option<TcpArguments> {
        Some(TcpArguments {
            host: self.host,
            port: self.port,
            timeout: Some(self.timeout),
        })
    }

    pub(crate) fn connect(
        &mut self,
    ) -> Task<
        Result<(
            Box<dyn AsyncWrite + Unpin + Send + 'static>,
            Box<dyn AsyncRead + Unpin + Send + 'static>,
        )>,
    > {
        let executor = self.executor.clone();
        let timeout = self.timeout;
        let address = SocketAddr::new(self.host, self.port);
        let process = self.process.clone();
        executor.clone().spawn(async move {
            select! {
                _ = executor.timer(Duration::from_millis(timeout)).fuse() => {
                    anyhow::bail!("Connection to TCP DAP timeout {address}");
                },
                result = executor.clone().spawn(async move {
                    loop {
                        match TcpStream::connect(address).await {
                            Ok(stream) => {
                                let (read, write) = stream.split();
                                return Ok((Box::new(write) as _, Box::new(read) as _))
                            },
                            Err(_) => {
                                let has_process = process.lock().is_some();
                                if has_process {
                                    let status = process.lock().as_mut().unwrap().try_status();
                                    if let Ok(Some(_)) = status {
                                        let child = process.lock().take().unwrap();
                                        let output = child.output().await?;
                                        let output = if output.stderr.is_empty() {
                                            String::from_utf8_lossy(&output.stdout).to_string()
                                        } else {
                                            String::from_utf8_lossy(&output.stderr).to_string()
                                        };
                                        anyhow::bail!("{output}\nerror: process exited before debugger attached.");
                                    }
                                }

                                executor.timer(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }).fuse() => result
            }
        })
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        if let Some(mut p) = self.process.lock().take() {
            p.kill().log_err();
        }
    }
}
