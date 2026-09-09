use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct ToolInfo {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct ToolCallRequest {
    pub(crate) arguments: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ToolResult {
    pub(crate) content: String,
    pub(crate) is_error: bool,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/tools",
    responses(
        (status = 200, description = "List all available tools", body = Vec<ToolInfo>)
    ),
    tag = "AAgent"
)]
pub(crate) async fn list_tools(State(state): State<AppState>) -> Json<Vec<ToolInfo>> {
    let registry = state.registry.read().await;
    let tools = registry
        .all_definitions()
        .into_iter()
        .map(|d| ToolInfo {
            name: d.name,
            description: d.description,
            parameters: d.parameters,
        })
        .collect();
    Json(tools)
}

#[utoipa::path(
    post,
    path = "/tools/{name}",
    params(
        ("name" = String, Path, description = "Tool name")
    ),
    request_body = ToolCallRequest,
    responses(
        (status = 200, description = "Tool execution result", body = ToolResult),
        (status = 404, description = "Tool not found", body = String),
        (status = 500, description = "Tool execution failed", body = String),
    ),
    tag = "AAgent"
)]
pub(crate) async fn call_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolResult>, (StatusCode, String)> {
    let ctx = aa_kernel::tool_provider::ToolExecutionContext {
        session_id: "http".into(),
        working_dir: ".".into(),
    };

    let tool = {
        let registry = state.registry.read().await;
        registry.find(&name).map(|t| t)
    }
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Tool '{name}' not found")))?;

    let result = tool.execute(req.arguments, &ctx).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Tool execution failed: {e}"),
        )
    })?;

    Ok(Json(ToolResult {
        content: result.content,
        is_error: result.is_error,
        metadata: result.metadata,
    }))
}
