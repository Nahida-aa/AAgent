use anyhow::{Context as _, Result, bail};
use futures::{AsyncRead, AsyncWrite};
use gpui::{AsyncApp, Task};
use parking_lot::Mutex;
use std::process::Stdio;

use crate::adapters::DebugAdapterBinary;
use util::{ResultExt, process::Child};

use super::delegate::TransportDelegate;
use super::transport_trait::Transport;
use super::types::{IoKind, LogHandlers};

pub struct StdioTransport {
    process: Mutex<Child>,
    _stderr_task: Option<gpui::Task<()>>,
}

impl StdioTransport {
    pub(crate) async fn start(
        binary: &DebugAdapterBinary,
        log_handlers: LogHandlers,
        cx: &mut AsyncApp,
    ) -> Result<Self> {
        let Some(binary_command) = &binary.command else {
            bail!(
                "When using the `stdio` transport, the path to a debug adapter binary must be set by Zed."
            );
        };
        let mut command = util::command::new_std_command(&binary_command);

        if let Some(cwd) = &binary.cwd {
            command.current_dir(cwd);
        }

        command.args(&binary.arguments);
        command.envs(&binary.envs);

        let mut process = Child::spawn(command, Stdio::piped(), Stdio::piped(), Stdio::piped())?;

        let _stderr_task = process.stderr.take().map(|stderr| {
            cx.background_spawn(TransportDelegate::handle_adapter_log(
                stderr,
                IoKind::StdErr,
                log_handlers,
            ))
        });

        let process = Mutex::new(process);

        Ok(Self {
            process,
            _stderr_task,
        })
    }
}

impl Transport for StdioTransport {
    pub(crate) fn has_adapter_logs(&self) -> bool { true }

    pub(crate) fn kill(&mut self) { self.process.lock().kill().log_err(); }

    pub(crate) fn connect(
        &mut self,
    ) -> Task<
        Result<(
            Box<dyn AsyncWrite + Unpin + Send + 'static>,
            Box<dyn AsyncRead + Unpin + Send + 'static>,
        )>,
    > {
        let result = util::maybe!({
            let mut process = self.process.lock();
            Ok((
                Box::new(process.stdin.take().context("Cannot reconnect")?) as _,
                Box::new(process.stdout.take().context("Cannot reconnect")?) as _,
            ))
        });
        Task::ready(result)
    }

    pub(crate) fn tcp_arguments(&self) -> Option<TcpArguments> { None }
}

impl Drop for StdioTransport {
    fn drop(&mut self) { self.process.lock().kill().log_err(); }
}
