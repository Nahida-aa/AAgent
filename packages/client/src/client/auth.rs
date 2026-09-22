use std::sync::Arc;

use futures::{FutureExt as _, StreamExt as _};
use gpui::AsyncApp;
use util::ResultExt as _;

use crate::credentials::Credentials;
use crate::env::{ADMIN_API_TOKEN, IMPERSONATE_LOGIN, USE_WEB_LOGIN};
use crate::status::Status;

use super::Client;

impl Client {
    pub async fn has_credentials(&self, cx: &AsyncApp) -> bool {
        self.credentials_provider
            .read_credentials(cx)
            .await
            .is_some()
    }

    pub async fn sign_in(
        self: &Arc<Self>,
        try_provider: bool,
        cx: &AsyncApp,
    ) -> Result<Credentials> {
        let is_reauthenticating = if self.status().borrow().is_signed_out() {
            self.set_status(Status::Authenticating, cx);
            false
        } else {
            self.set_status(Status::Reauthenticating, cx);
            true
        };

        let mut credentials = None;

        let old_credentials = self.state.read().credentials.clone();
        if let Some(old_credentials) = old_credentials
            && self.validate_credentials(&old_credentials, cx).await?
        {
            credentials = Some(old_credentials);
        }

        if credentials.is_none()
            && try_provider
            && let Some(stored_credentials) = self.credentials_provider.read_credentials(cx).await
        {
            if self.validate_credentials(&stored_credentials, cx).await? {
                credentials = Some(stored_credentials);
            } else {
                self.credentials_provider
                    .delete_credentials(cx)
                    .await
                    .log_err();
            }
        }

        if credentials.is_none() {
            let mut status_rx = self.status();
            let _ = status_rx.next().await;
            futures::select_biased! {
                authenticate = self.authenticate(cx).fuse() => {
                    match authenticate {
                        Ok(creds) => {
                            if IMPERSONATE_LOGIN.is_none() {
                                self.credentials_provider
                                    .write_credentials(creds.user_id, creds.access_token.clone(), cx)
                                    .await
                                    .log_err();
                            }

                            credentials = Some(creds);
                        },
                        Err(err) => {
                            self.set_status(Status::AuthenticationError, cx);
                            return Err(err);
                        }
                    }
                }
                _ = status_rx.next().fuse() => {
                    return Err(anyhow!("authentication canceled"));
                }
            }
        }

        let credentials = credentials.unwrap();
        self.set_id(credentials.user_id);
        self.cloud_client
            .set_credentials(credentials.user_id as u32, credentials.access_token.clone());
        self.state.write().credentials = Some(credentials.clone());
        self.set_status(
            if is_reauthenticating {
                Status::Reauthenticated
            } else {
                Status::Authenticated
            },
            cx,
        );

        Ok(credentials)
    }

    async fn validate_credentials(
        self: &Arc<Self>,
        credentials: &Credentials,
        cx: &AsyncApp,
    ) -> Result<bool> {
        match self
            .cloud_client
            .validate_credentials(credentials.user_id as u32, &credentials.access_token)
            .await
        {
            Ok(valid) => Ok(valid),
            Err(err) => {
                self.set_status(Status::AuthenticationError, cx);
                Err(err.context("failed to validate credentials"))
            }
        }
    }

    /// Performs a sign-in and also (optionally) connects to Collab.
    ///
    /// Only Zed staff automatically connect to Collab.
    pub async fn sign_in_with_optional_connect(
        self: &Arc<Self>,
        try_provider: bool,
        cx: &AsyncApp,
    ) -> Result<()> {
        // Don't try to sign in again if we're already connected to Collab, as it will temporarily disconnect us.
        if self.status().borrow().is_connected() {
            return Ok(());
        }

        let (is_staff_tx, is_staff_rx) = oneshot::channel::<bool>();
        let mut is_staff_tx = Some(is_staff_tx);
        cx.update(|cx| {
            cx.on_flags_ready(move |state, _cx| {
                if let Some(is_staff_tx) = is_staff_tx.take() {
                    is_staff_tx.send(state.is_staff).log_err();
                }
            })
            .detach();
        });

        let credentials = self.sign_in(try_provider, cx).await?;

        self.connect_to_cloud(cx);

        cx.update(move |cx| {
            cx.spawn({
                let client = self.clone();
                async move |cx| {
                    let is_staff = is_staff_rx.await?;
                    if is_staff {
                        match client.connect_with_credentials(credentials, cx).await {
                            ConnectionResult::Timeout => Err(anyhow!("connection timed out")),
                            ConnectionResult::ConnectionReset => Err(anyhow!("connection reset")),
                            ConnectionResult::Result(result) => {
                                result.context("client auth and connect")
                            }
                        }
                    } else {
                        Ok(())
                    }
                }
            })
            .detach_and_log_err(cx);
        });

        Ok(())
    }

    fn authenticate(self: &Arc<Self>, cx: &AsyncApp) -> Task<Result<Credentials>> {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(callback) = self.authenticate.read().as_ref() {
            return callback(cx);
        }

        self.authenticate_with_browser(cx)
    }

    pub fn authenticate_with_browser(self: &Arc<Self>, cx: &AsyncApp) -> Task<Result<Credentials>> {
        let http = self.http.clone();
        let this = self.clone();
        cx.spawn(async move |cx| {
            let background = cx.background_executor().clone();

            let (open_url_tx, open_url_rx) = oneshot::channel::<String>();
            cx.update(|cx| {
                cx.spawn(async move |cx| {
                    if let Ok(url) = open_url_rx.await {
                        cx.update(|cx| cx.open_url(&url));
                    }
                })
                .detach();
            });

            let credentials = background
                .clone()
                .spawn(async move {
                    // Generate a pair of asymmetric encryption keys. The public key will be used by the
                    // zed server to encrypt the user's access token, so that it can'be intercepted by
                    // any other app running on the user's device.
                    let (public_key, private_key) =
                        rpc::auth::keypair().context("failed to generate keypair for auth")?;
                    let public_key = String::try_from(public_key)
                        .context("failed to serialize public key for auth")?;

                    if let Some((login, token)) =
                        IMPERSONATE_LOGIN.as_ref().zip(ADMIN_API_TOKEN.as_ref())
                    {
                        if !*USE_WEB_LOGIN {
                            eprintln!("authenticate as admin {login}, {token}");

                            return this
                                .authenticate_as_admin(http, login.clone(), token.clone())
                                .await;
                        }
                    }

                    // Start an HTTP server to receive the redirect from Zed's sign-in page.
                    let server = tiny_http::Server::http("127.0.0.1:0")
                        .map_err(|e| anyhow!(e).context("failed to bind callback port"))?;
                    let port = server
                        .server_addr()
                        .to_ip()
                        .context("server not bound to a TCP address")?
                        .port();

                    #[derive(Serialize)]
                    struct NativeAppSignInQueryParams {
                        native_app_port: u16,
                        native_app_public_key: String,
                        system_id: Option<Arc<str>>,
                    }

                    // Open the Zed sign-in page in the user's browser, with query parameters that indicate
                    // that the user is signing in from a Zed app running on the same device.
                    let url = http.build_url(&format!(
                        "/native_app_signin?{}",
                        serde_urlencoded::to_string(&NativeAppSignInQueryParams {
                            native_app_port: port,
                            native_app_public_key: public_key,
                            system_id: this.telemetry.system_id(),
                        })?
                    ));

                    open_url_tx.send(url).log_err();

                    #[derive(Deserialize)]
                    struct CallbackParams {
                        pub user_id: String,
                        pub access_token: String,
                    }

                    // Receive the HTTP request from the user's browser. Retrieve the user id and encrypted
                    // access token from the query params.
                    //
                    // TODO - Avoid ever starting more than one HTTP server. Maybe switch to using a
                    // custom URL scheme instead of this local HTTP server.
                    let (user_id, access_token) = background
                        .spawn(async move {
                            for _ in 0..100 {
                                if let Some(req) = server.recv_timeout(Duration::from_secs(1))? {
                                    let path = req.url();
                                    let url = Url::parse(&format!("http://example.com{}", path))
                                        .context("failed to parse login notification url")?;
                                    let callback_params: CallbackParams =
                                        serde_urlencoded::from_str(url.query().unwrap_or_default())
                                            .context(
                                                "failed to parse sign-in callback query parameters",
                                            )?;

                                    let post_auth_url =
                                        http.build_url("/native_app_signin_succeeded");
                                    req.respond(
                                        tiny_http::Response::empty(302).with_header(
                                            tiny_http::Header::from_bytes(
                                                &b"Location"[..],
                                                post_auth_url.as_bytes(),
                                            )
                                            .unwrap(),
                                        ),
                                    )
                                    .context("failed to respond to login http request")?;
                                    return Ok((
                                        callback_params.user_id,
                                        callback_params.access_token,
                                    ));
                                }
                            }

                            anyhow::bail!("didn't receive login redirect");
                        })
                        .await?;

                    let access_token = private_key
                        .decrypt_string(&access_token)
                        .context("failed to decrypt access token")?;

                    Ok(Credentials {
                        user_id: user_id.parse()?,
                        access_token,
                    })
                })
                .await?;

            cx.update(|cx| cx.activate(true));
            Ok(credentials)
        })
    }

    async fn authenticate_as_admin(
        self: &Arc<Self>,
        http: Arc<HttpClientWithUrl>,
        login: String,
        api_token: String,
    ) -> Result<Credentials> {
        #[derive(Serialize)]
        struct ImpersonateUserBody {
            github_login: String,
        }

        #[derive(Deserialize)]
        struct ImpersonateUserResponse {
            user_id: u64,
            access_token: String,
        }

        let url = self
            .http
            .build_zed_cloud_url("/internal/users/impersonate")?;
        let request = Request::post(url.as_str())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {api_token}"))
            .body(
                serde_json::to_string(&ImpersonateUserBody {
                    github_login: login,
                })?
                .into(),
            )?;

        let mut response = http.send(request).await?;
        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "admin user request failed {} - {}",
            response.status().as_u16(),
            body,
        );
        let response: ImpersonateUserResponse = serde_json::from_str(&body)?;

        Ok(Credentials {
            user_id: response.user_id,
            access_token: response.access_token,
        })
    }

    pub async fn sign_out(self: &Arc<Self>, cx: &AsyncApp) {
        self.state.write().credentials = None;
        self.cloud_client.clear_credentials();
        self.disconnect(cx);

        if self.has_credentials(cx).await {
            self.credentials_provider
                .delete_credentials(cx)
                .await
                .log_err();
        }
    }

    /// Requests a sign out to be performed asynchronously.
    pub fn request_sign_out(&self) {
        if let Some(sign_out_tx) = self.sign_out_tx.lock().clone() {
            sign_out_tx.unbounded_send(()).ok();
        }
    }
}
