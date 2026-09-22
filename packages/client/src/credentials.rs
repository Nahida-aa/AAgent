use std::future::Future;
use std::pin::Pin;

use credentials_provider::CredentialsProvider;
use gpui::{App, AsyncApp};
use util::ResultExt as _;

use crate::env::IMPERSONATE_LOGIN;
use crate::settings::ClientSettings;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    pub user_id: u64,
    pub access_token: String,
}

impl Credentials {
    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.user_id, self.access_token)
    }
}

pub(crate) struct ClientCredentialsProvider {
    pub(crate) provider: std::sync::Arc<dyn CredentialsProvider>,
}

impl ClientCredentialsProvider {
    pub fn new(cx: &App) -> Self {
        Self {
            provider: ad_credentials_provider::global(cx),
        }
    }

    fn server_url(&self, cx: &AsyncApp) -> anyhow::Result<String> {
        Ok(cx.update(|cx| ClientSettings::get_global(cx).server_url.clone()))
    }

    fn credentials_url(&self, cx: &AsyncApp) -> anyhow::Result<String> {
        let from_settings = cx.update(|cx| ClientSettings::get_global(cx).credentials_url.clone());
        Ok(from_settings.unwrap_or(self.server_url(cx)?))
    }

    pub(crate) fn read_credentials<'a>(
        &'a self,
        cx: &'a AsyncApp,
    ) -> Pin<Box<dyn Future<Output = Option<Credentials>> + 'a>> {
        async move {
            if IMPERSONATE_LOGIN.is_some() {
                return None;
            }

            let credentials_url = self.credentials_url(cx).ok()?;
            let (user_id, access_token) = self
                .provider
                .read_credentials(&credentials_url, cx)
                .await
                .log_err()
                .flatten()?;

            Some(Credentials {
                user_id: user_id.parse().ok()?,
                access_token: String::from_utf8(access_token).ok()?,
            })
        }
        .boxed_local()
    }

    pub(crate) fn write_credentials<'a>(
        &'a self,
        user_id: u64,
        access_token: String,
        cx: &'a AsyncApp,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>> {
        async move {
            let credentials_url = self.credentials_url(cx)?;
            self.provider
                .write_credentials(
                    &credentials_url,
                    &user_id.to_string(),
                    access_token.as_bytes(),
                    cx,
                )
                .await
        }
        .boxed_local()
    }

    /// Deletes the credentials from the provider.
    pub(crate) fn delete_credentials<'a>(
        &'a self,
        cx: &'a AsyncApp,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>> {
        async move {
            let credentials_url = self.credentials_url(cx)?;
            self.provider.delete_credentials(&credentials_url, cx).await
        }
        .boxed_local()
    }
}
