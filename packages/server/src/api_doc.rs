use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        super::health::health,
        super::tools::list_tools,
        super::tools::call_tool,
        super::sessions::list_sessions,
        super::sessions::delete_session,
    ),
    components(schemas(
        super::health::HealthResponse,
        super::tools::ToolInfo,
        super::tools::ToolCallRequest,
        super::tools::ToolResult,
        super::chat::AGUIChatRequest,
        super::chat::ChatMsg,
        super::chat::ToolDef,
        super::chat::ToolCallWire,
        super::chat::ToolCallFuncWire,
        super::chat::SseEvent,
        super::chat::TextPart,
        super::chat::FilePart,
        super::chat::FilePartSource,
        super::chat::PartTextRange,
        super::chat::AgentPart,
        super::chat::AgentPartSource,
        super::chat::PartTime,
        super::sessions::SessionSummary,
    )),
    tags(
        (name = "AAgent", description = "AAgent API")
    )
)]
pub(crate) struct ApiDoc;

pub(crate) async fn openapi_json() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::to_value(ApiDoc::openapi()).unwrap())
}
