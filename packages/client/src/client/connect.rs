use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use futures::{FutureExt as _, SinkExt as _, StreamExt as _, TryStreamExt as _};
use gpui::{AsyncApp, Task};
use async_tungstenite::tungstenite::client::IntoClientRequest;
use http_client::{HttpClientWithUrl, http, http::HeaderValue};
use release_channel::{AppVersion, ReleaseChannel};
use rpc::proto;
use rpc::{Connection, Peer, TypedEnvelope};
use tokio::net::TcpStream;
use url::Url;
use util::ConnectionResult;

use crate::proxy::{connect_proxy_stream, excluded_from_proxy};

use crate::constants::CONNECTION_TIMEOUT;
use crate::error::EstablishConnectionError;
use crate::status::Status;
use crate::{Credentials, ZED_RPC_URL};

use super::Client;

impl Client {
    pub async fn connect(
        self: &Arc<Self>,
        try_provider: bool,
        cx: &AsyncApp,
    ) -> ConnectionResult<()> {
        let was_disconnected = match *self.status().borrow() {
            Status::SignedOut | Status::Authenticated => true,
            Status::ConnectionError
            | Status::ConnectionLost
            | Status::Authenticating
            | Status::AuthenticationError
            | Status::Reauthenticating
            | Status::Reauthenticated
            | Status::ReconnectionError { .. } => false,
            Status::Connected { .. } | Status::Connecting | Status::Reconnecting => {
                return ConnectionResult::Result(Ok(()));
            }
            Status::UpgradeRequired => {
                return ConnectionResult::Result(
                    Err(EstablishConnectionError::UpgradeRequired)
                        .context("client auth and connect"),
                );
            }
        };
        let credentials = match self.sign_in(try_provider, cx).await {
            Ok(credentials) => credentials,
            Err(err) => return ConnectionResult::Result(Err(err)),
        };

        if was_disconnected {
            self.set_status(Status::Connecting, cx);
        } else {
            self.set_status(Status::Reconnecting, cx);
        }

        self.connect_with_credentials(credentials, cx).await
    }

    pub(crate) async fn connect_with_credentials(
        self: &Arc<Self>,
        credentials: Credentials,
        cx: &AsyncApp,
    ) -> ConnectionResult<()> {
        let mut timeout =
            futures::FutureExt::fuse(cx.background_executor().timer(CONNECTION_TIMEOUT));
        futures::select_biased! {
            connection = self.establish_connection(&credentials, cx).fuse() => {
                match connection {
                    Ok(conn) => {
                        futures::select_biased! {
                            result = self.set_connection(conn, cx).fuse() => {
                                match result.context("client auth and connect") {
                                    Ok(()) => ConnectionResult::Result(Ok(())),
                                    Err(err) => {
                                        self.set_status(Status::ConnectionError, cx);
                                        ConnectionResult::Result(Err(err))
                                    },
                                }
                            },
                            _ = timeout => {
                                self.set_status(Status::ConnectionError, cx);
                                ConnectionResult::Timeout
                            }
                        }
                    }
                    Err(EstablishConnectionError::Unauthorized) => {
                        self.set_status(Status::ConnectionError, cx);
                        ConnectionResult::Result(Err(EstablishConnectionError::Unauthorized).context("client auth and connect"))
                    }
                    Err(EstablishConnectionError::UpgradeRequired) => {
                        self.set_status(Status::UpgradeRequired, cx);
                        ConnectionResult::Result(Err(EstablishConnectionError::UpgradeRequired).context("client auth and connect"))
                    }
                    Err(error) => {
                        self.set_status(Status::ConnectionError, cx);
                        ConnectionResult::Result(Err(error).context("client auth and connect"))
                    }
                }
            }
            _ = &mut timeout => {
                self.set_status(Status::ConnectionError, cx);
                ConnectionResult::Timeout
            }
        }
    }

    pub(crate) async fn set_connection(
        self: &Arc<Self>,
        conn: Connection,
        cx: &AsyncApp,
    ) -> Result<()> {
        let executor = cx.background_executor();
        log::debug!("add connection to peer");
        let (connection_id, handle_io, mut incoming) = self.peer.add_connection(conn, {
            let executor = executor.clone();
            move |duration| executor.timer(duration)
        });
        let handle_io = executor.spawn(handle_io);

        let peer_id = async {
            log::debug!("waiting for server hello");
            let message = incoming.next().await.context("no hello message received")?;
            log::debug!("got server hello");
            let hello_message_type_name = message.payload_type_name().to_string();
            let hello = message
                .into_any()
                .downcast::<TypedEnvelope<proto::Hello>>()
                .map_err(|_| {
                    anyhow!(
                        "invalid hello message received: {:?}",
                        hello_message_type_name
                    )
                })?;
            let peer_id = hello.payload.peer_id.context("invalid peer id")?;
            Ok(peer_id)
        };

        let peer_id = match peer_id.await {
            Ok(peer_id) => peer_id,
            Err(error) => {
                self.peer.disconnect(connection_id);
                return Err(error);
            }
        };

        log::debug!(
            "set status to connected (connection id: {:?}, peer id: {:?})",
            connection_id,
            peer_id
        );
        self.set_status(
            Status::Connected {
                peer_id,
                connection_id,
            },
            cx,
        );

        cx.spawn({
            let this = self.clone();
            async move |cx| {
                while let Some(message) = incoming.next().await {
                    this.handle_message(message, cx);
                    // Don't starve the main thread when receiving lots of messages at once.
                    smol::future::yield_now().await;
                }
            }
        })
        .detach();

        cx.spawn({
            let this = self.clone();
            async move |cx| match handle_io.await {
                Ok(()) => {
                    if *this.status().borrow()
                        == (Status::Connected {
                            connection_id,
                            peer_id,
                        })
                    {
                        this.set_status(Status::SignedOut, cx);
                    }
                }
                Err(err) => {
                    log::error!("connection error: {:?}", err);
                    this.set_status(Status::ConnectionLost, cx);
                }
            }
        })
        .detach();

        Ok(())
    }

    pub(crate) fn establish_connection(
        self: &Arc<Self>,
        credentials: &Credentials,
        cx: &AsyncApp,
    ) -> gpui::Task<Result<Connection, EstablishConnectionError>> {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(callback) = self.establish_connection.read().as_ref() {
            return callback(credentials, cx);
        }

        self.establish_websocket_connection(credentials, cx)
    }

    fn rpc_url(
        &self,
        http: Arc<HttpClientWithUrl>,
        release_channel: Option<ReleaseChannel>,
    ) -> impl Future<Output = Result<url::Url>> + use<> {
        #[cfg(any(test, feature = "test-support"))]
        let url_override = self.rpc_url.read().clone();

        async move {
            #[cfg(any(test, feature = "test-support"))]
            if let Some(url) = url_override {
                return Ok(url);
            }

            if let Some(url) = &*ZED_RPC_URL {
                return Url::parse(url).context("invalid rpc url");
            }

            let mut url = http.build_url("/rpc");
            if let Some(preview_param) =
                release_channel.and_then(|channel| channel.release_query_param())
            {
                url += "?";
                url += preview_param;
            }

            let response = http.get(&url, Default::default(), false).await?;
            anyhow::ensure!(
                response.status().is_redirection(),
                "unexpected /rpc response status {}",
                response.status()
            );
            let collab_url = response
                .headers()
                .get("Location")
                .context("missing location header in /rpc response")?
                .to_str()
                .map_err(EstablishConnectionError::other)?
                .to_string();
            Url::parse(&collab_url).with_context(|| format!("parsing collab rpc url {collab_url}"))
        }
    }

    fn establish_websocket_connection(
        self: &Arc<Self>,
        credentials: &Credentials,
        cx: &AsyncApp,
    ) -> Task<Result<Connection, EstablishConnectionError>> {
        let release_channel = cx.update(|cx| ReleaseChannel::try_global(cx));
        let app_version = cx.update(|cx| AppVersion::global(cx).to_string());

        let http = self.http.clone();
        let proxy = http.proxy().cloned();
        let user_agent = http.user_agent().cloned();
        let credentials = credentials.clone();
        let rpc_url = self.rpc_url(http, release_channel);
        let system_id = self.telemetry.system_id();
        let metrics_id = self.telemetry.metrics_id();
        cx.spawn(async move |cx| {
            use HttpOrHttps::*;

            #[derive(Debug)]
            enum HttpOrHttps {
                Http,
                Https,
            }

            let mut rpc_url = rpc_url.await?;
            let url_scheme = match rpc_url.scheme() {
                "https" => Https,
                "http" => Http,
                _ => Err(anyhow!("invalid rpc url: {}", rpc_url))?,
            };

            let stream = gpui_tokio::Tokio::spawn_result(cx, {
                let rpc_url = rpc_url.clone();
                async move {
                    let rpc_host = rpc_url
                        .host_str()
                        .zip(rpc_url.port_or_known_default())
                        .context("missing host in rpc url")?;
                    Ok(match proxy {
                        Some(proxy) if !excluded_from_proxy(rpc_host.0) => {
                            connect_proxy_stream(&proxy, rpc_host).await?
                        }
                        _ => Box::new(TcpStream::connect(rpc_host).await?),
                    })
                }
            })
            .await?;

            log::info!("connected to rpc endpoint {}", rpc_url);

            rpc_url
                .set_scheme(match url_scheme {
                    Https => "wss",
                    Http => "ws",
                })
                .unwrap();

            // We call `into_client_request` to let `tungstenite` construct the WebSocket request
            // for us from the RPC URL.
            //
            // Among other things, it will generate and set a `Sec-WebSocket-Key` header for us.
            let mut request = IntoClientRequest::into_client_request(rpc_url.as_str())?;

            // We then modify the request to add our desired headers.
            let request_headers = request.headers_mut();
            request_headers.insert(
                http::header::AUTHORIZATION,
                HeaderValue::from_str(&credentials.authorization_header())?,
            );
            request_headers.insert(
                "x-zed-protocol-version",
                HeaderValue::from_str(&rpc::PROTOCOL_VERSION.to_string())?,
            );
            request_headers.insert("x-zed-app-version", HeaderValue::from_str(&app_version)?);
            request_headers.insert(
                "x-zed-release-channel",
                HeaderValue::from_str(release_channel.map(|r| r.dev_name()).unwrap_or("unknown"))?,
            );
            if let Some(user_agent) = user_agent {
                request_headers.insert(http::header::USER_AGENT, user_agent);
            }
            if let Some(system_id) = system_id {
                request_headers.insert("x-zed-system-id", HeaderValue::from_str(&system_id)?);
            }
            if let Some(metrics_id) = metrics_id {
                request_headers.insert("x-zed-metrics-id", HeaderValue::from_str(&metrics_id)?);
            }

            let (stream, _) = async_tungstenite::tokio::client_async_tls_with_connector_and_config(
                request,
                stream,
                Some(Arc::new(http_client_tls::tls_config()).into()),
                None,
            )
            .await?;

            Ok(Connection::new(
                stream
                    .map_err(|error| anyhow!(error))
                    .sink_map_err(|error| anyhow!(error)),
            ))
        })
    }
}
