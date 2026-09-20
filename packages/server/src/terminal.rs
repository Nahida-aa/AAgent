use collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use futures_util::{SinkExt, StreamExt};
use nix::fcntl::OFlag;
use nix::pty::{self, Winsize};
use nix::unistd;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, mpsc};
use uuid::Uuid;

const DEFAULT_SHELL: &str = "sh";

pub struct PtySession {
    master_fd: nix::pty::PtyMaster,
    child: std::process::Child,
}

impl PtySession {
    pub fn spawn(shell: Option<&str>) -> io::Result<Self> {
        let shell = shell.unwrap_or(DEFAULT_SHELL);

        let master = pty::posix_openpt(OFlag::O_RDWR | OFlag::O_NONBLOCK)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        pty::grantpt(&master).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        pty::unlockpt(&master).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let slave_name = unsafe { pty::ptsname(&master) }
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let slave = std::fs::File::options()
            .read(true)
            .write(true)
            .open(std::path::Path::new(&slave_name))?;
        let slave_fd = slave.as_raw_fd();

        let fd_in = unistd::dup(slave_fd).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let fd_out = unistd::dup(slave_fd).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let fd_err = unistd::dup(slave_fd).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        drop(slave);

        let child = std::process::Command::new(shell)
            .stdin(unsafe { std::process::Stdio::from_raw_fd(fd_in) })
            .stdout(unsafe { std::process::Stdio::from_raw_fd(fd_out) })
            .stderr(unsafe { std::process::Stdio::from_raw_fd(fd_err) })
            .spawn()?;

        Ok(PtySession {
            master_fd: master,
            child,
        })
    }

    pub fn raw_fd(&self) -> RawFd { self.master_fd.as_raw_fd() }

    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        let ret = unsafe {
            libc::write(
                self.master_fd.as_raw_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
            )
        };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let ret = unsafe { libc::ioctl(self.master_fd.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone)]
pub struct TerminalManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<Mutex<PtySession>>>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn list(&self) -> Vec<Uuid> { self.sessions.read().await.keys().copied().collect() }

    pub async fn create(&self, shell: Option<&str>) -> io::Result<Uuid> {
        let session = PtySession::spawn(shell)?;
        let id = Uuid::new_v4();
        let mut sessions = self.sessions.write().await;
        sessions.insert(id, Arc::new(Mutex::new(session)));
        Ok(id)
    }

    pub async fn remove(&self, id: &Uuid) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id);
    }

    pub async fn attach(&self, socket: WebSocket, shell: Option<&str>) {
        let id = match self.create(shell).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("terminal: failed to spawn PTY: {e}");
                return;
            }
        };
        PtyBridge::spawn(socket, self.clone(), id, true).await;
    }

    /// Connect a WebSocket to an existing terminal session.
    /// Unlike `attach`, the session is NOT removed when the WS closes.
    pub async fn connect(&self, socket: WebSocket, id: Uuid) {
        let exists = self.sessions.read().await.contains_key(&id);
        if !exists {
            tracing::warn!("terminal: session {id} not found");
            return;
        }
        PtyBridge::spawn(socket, self.clone(), id, false).await;
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum TerminalControl {
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
}

pub struct PtyBridge;

impl PtyBridge {
    pub async fn spawn(
        socket: WebSocket,
        manager: TerminalManager,
        id: Uuid,
        remove_on_close: bool,
    ) {
        let (mut sender, mut receiver) = socket.split();

        let dup_fd = {
            let sessions = manager.sessions.read().await;
            let session = sessions.get(&id).unwrap();
            let s = session.lock().await;
            unistd::dup(s.raw_fd()).unwrap()
        };

        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1024);

        let reader = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 8192];
            loop {
                let n =
                    unsafe { libc::read(dup_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    break;
                }
                if tx.blocking_send(buf[..n as usize].to_vec()).is_err() {
                    break;
                }
            }
            unsafe {
                libc::close(dup_fd);
            }
        });

        let writer = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(data) = rx.recv() => {
                        if sender.send(Message::Binary(data.into())).await.is_err() {
                            break;
                        }
                    }
                    msg = receiver.next() => {
                        match msg {
                            Some(Ok(Message::Binary(data))) => {
                                let sessions = manager.sessions.read().await;
                                if let Some(session) = sessions.get(&id) {
                                    let s = session.lock().await;
                                    s.write(&data).ok();
                                }
                            }
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(ctrl) = serde_json::from_str::<TerminalControl>(&text) {
                                    match ctrl {
                                        TerminalControl::Resize { cols, rows } => {
                                            let sessions = manager.sessions.read().await;
                                            if let Some(session) = sessions.get(&id) {
                                                let s = session.lock().await;
                                                s.resize(cols, rows).ok();
                                            }
                                        }
                                    }
                                } else {
                                    // Not a control message, treat as terminal input
                                    let sessions = manager.sessions.read().await;
                                    if let Some(session) = sessions.get(&id) {
                                        let s = session.lock().await;
                                        s.write(text.as_bytes()).ok();
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                            Some(Ok(Message::Ping(payload))) => {
                                let _ = sender.send(Message::Pong(payload)).await;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if remove_on_close {
                manager.remove(&id).await;
            }
        });

        let _ = tokio::join!(writer);
        let _ = reader.await;
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        let manager = state.terminal.clone();
        async move {
            manager.attach(socket, None).await;
        }
    })
}

pub async fn ws_session_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<Uuid>,
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        let manager = state.terminal.clone();
        async move {
            manager.connect(socket, id).await;
        }
    })
}

#[derive(Serialize)]
pub(crate) struct SessionList {
    pub(crate) sessions: Vec<String>,
}

pub(crate) async fn list_sessions(State(state): State<crate::AppState>) -> Json<SessionList> {
    let ids = state.terminal.list().await;
    Json(SessionList {
        sessions: ids.iter().map(|u| u.to_string()).collect(),
    })
}

#[derive(Serialize)]
pub(crate) struct CreateSessionResponse {
    pub(crate) id: String,
}

pub(crate) async fn create_session(
    State(state): State<crate::AppState>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let id = state
        .terminal
        .create(None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CreateSessionResponse { id: id.to_string() }))
}

pub(crate) async fn delete_session(
    Path(id): Path<Uuid>,
    State(state): State<crate::AppState>,
) -> StatusCode {
    state.terminal.remove(&id).await;
    StatusCode::NO_CONTENT
}
