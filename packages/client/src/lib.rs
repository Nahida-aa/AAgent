#[cfg(any(test, feature = "test-support"))]
pub mod test;

mod actions;
mod client;
mod constants;
mod credentials;
mod env;
mod error;
mod settings;
mod status;
mod subscription;
mod zed_link;

mod llm_token;
mod proxy;
pub mod telemetry;
pub mod user;
pub mod zed_urls;

#[cfg(test)]
mod tests;

pub use actions::init;
pub use client::Client;
pub use constants::{CONNECTION_TIMEOUT, INITIAL_RECONNECTION_DELAY, MAX_RECONNECTION_DELAY};
pub use credentials::Credentials;
pub use env::{ADMIN_API_TOKEN, IMPERSONATE_LOGIN, USE_WEB_LOGIN, ZED_ALWAYS_ACTIVE, ZED_APP_PATH};
pub use error::EstablishConnectionError;
pub use llm_token::*;
pub use rpc::*;
pub use settings::{ClientSettings, ProxySettings, TelemetrySettings};
pub use status::Status;
pub use subscription::{
    GlobalClient, MessageToClientHandler, PendingEntitySubscription, Subscription,
};
pub use telemetry_events::Event;
pub use user::*;
pub use zed_link::{ZED_URL_SCHEME, ZedLink, parse_zed_link};

// 私有静态量仍然在 crate 根可见（同名或限定路径）
pub(crate) use env::{ZED_RPC_URL, ZED_SERVER_URL};
