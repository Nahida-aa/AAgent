use std::sync::Arc;

use cloud_api_client::{ClientApiError, LlmApiToken};
use cloud_api_types::OrganizationId;

use super::Client;

impl Client {
    pub async fn cached_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> anyhow::Result<String> { /* 原样 */
    }

    pub async fn authenticated_llm_request(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
        build_request: impl Fn(&str) -> anyhow::Result<http_client::Request<http_client::AsyncBody>>,
    ) -> anyhow::Result<http_client::Response<http_client::AsyncBody>> { /* 原样 */
    }

    pub async fn refresh_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> anyhow::Result<String> { /* 原样 */
    }

    pub async fn clear_and_refresh_llm_token(
        &self,
        llm_token: &LlmApiToken,
        organization_id: OrganizationId,
    ) -> anyhow::Result<String> { /* 原样 */
    }
}
