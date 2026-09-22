//! Remote client: connects to a remote host, multiplexes RPC over a transport,
//! tracks connection state, and reconnects on failure.

mod channel_client;
mod client;
mod connect;
mod connection;
mod delegate;
mod options;
mod platform;
mod state;

#[cfg(target_os = "windows")]
mod wsl_path;

#[cfg(test)]
mod tests;

pub use channel_client::ChannelClient;
pub use client::{RemoteClient, RemoteClientEvent};
pub use connect::{ConnectionPool, ConnectionPoolEntry, connect, has_active_connection};
pub use connection::{ConnectionIdentifier, RemoteConnection};
pub use delegate::RemoteClientDelegate;
pub use options::RemoteConnectionOptions;
pub use platform::{CommandTemplate, Interactive, RemoteArch, RemoteOs, RemotePlatform};
pub use state::{
    ConnectionState, HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT, INITIAL_CONNECTION_TIMEOUT,
    MAX_MISSED_HEARTBEATS, MAX_RECONNECT_ATTEMPTS,
};

#[cfg(target_os = "windows")]
pub use wsl_path::OpenWslPath;
