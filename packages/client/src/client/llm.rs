use std::sync::Arc;

use anyhow::Context as _;
use anyhow::Result;
use cloud_api_client::{ClientApiError, LlmApiToken};
use cloud_api_types::OrganizationId;

use crate::llm_token::NeedsLlmTokenRefresh as _;

use super::Client;

impl Client {
    pub async fn cached_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> Result<String> {
        let system_id = self.telemetry().system_id().map(|x| x.to_string());
        let cloud_client = self.cloud_client();
        match llm_token
            .cached(&cloud_client, system_id, organization_id)
            .await
        {
            Ok(token) => Ok(token),
            Err(ClientApiError::Unauthorized) => {
                self.request_sign_out();
                Err(ClientApiError::Unauthorized).context("Failed to create LLM token")
            }
            Err(err) => Err(anyhow::Error::from(err)),
        }
    }

    /// Sends an authenticated request to the Zed LLM service, retrying once
    /// with a refreshed token if the server signals that the cached LLM
    /// token is expired or otherwise rejected. Returns the raw response so
    /// callers can inspect headers and stream the body.
    pub async fn authenticated_llm_request(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
        build_request: impl Fn(&str) -> Result<http_client::Request<http_client::AsyncBody>>,
    ) -> Result<http_client::Response<http_client::AsyncBody>> {
        let http_client = self.http_client();
        let token = self
            .cached_llm_token(llm_token, organization_id.clone())
            .await?;
        let response = http_client.send(build_request(&token)?).await?;
        if !response.needs_llm_token_refresh()
            && response.status() != http_client::http::StatusCode::UNAUTHORIZED
        {
            return Ok(response);
        }
        log::info!("LLM token rejected; refreshing and retrying request");
        let token = self.refresh_llm_token(llm_token, organization_id).await?;
        http_client.send(build_request(&token)?).await
    }

    pub async fn refresh_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> Result<String> {
        let system_id = self.telemetry().system_id().map(|x| x.to_string());
        let cloud_client = self.cloud_client();
        match llm_token
            .refresh(&cloud_client, system_id, organization_id)
            .await
        {
            Ok(token) => Ok(token),
            Err(ClientApiError::Unauthorized) => {
                self.request_sign_out();
                return Err(ClientApiError::Unauthorized).context("Failed to create LLM token");
            }
            Err(err) => return Err(anyhow::Error::from(err)),
        }
    }

    pub async fn clear_and_refresh_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> Result<String> {
        let system_id = self.telemetry().system_id().map(|x| x.to_string());
        let cloud_client = self.cloud_client();
        match llm_token
            .clear_and_refresh(&cloud_client, system_id, organization_id)
            .await
        {
            Ok(token) => Ok(token),
            Err(ClientApiError::Unauthorized) => {
                self.request_sign_out();
                return Err(ClientApiError::Unauthorized).context("Failed to create LLM token");
            }
            Err(err) => return Err(anyhow::Error::from(err)),
        }
    }
}
