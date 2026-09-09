use axum::extract::State;
use axum::response::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct HealthResponse {
    pub(crate) status: String,
    pub(crate) tool_count: usize,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) mcp: usize,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server health status", body = HealthResponse)
    ),
    tag = "AAgent"
)]
pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let count = state.registry.read().await.len();
    Json(HealthResponse {
        status: "ok".to_string(),
        tool_count: count,
        provider: state.resolved.provider.clone(),
        model: state.resolved.model.clone(),
        mcp: state.mcp_count,
    })
}
