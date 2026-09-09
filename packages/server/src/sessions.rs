use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(crate) struct SessionSummary {
    session_id: String,
    model: String,
    provider: String,
    message_count: usize,
    updated_at: String,
    created_at: String,
}

#[derive(Serialize)]
pub(crate) struct SessionDetail {
    session_id: String,
    model: String,
    provider: String,
    messages: Vec<aa_core::llm::Message>,
    created_at: String,
    updated_at: String,
}

#[utoipa::path(
    get,
    path = "/sessions",
    responses(
        (status = 200, description = "List saved sessions", body = Vec<SessionSummary>)
    ),
    tag = "AAgent"
)]
/// List saved sessions (most recent first).
pub(crate) async fn list_sessions() -> Json<Vec<SessionSummary>> {
    let sessions = aa_session::storage::list().unwrap_or_default();
    Json(
        sessions
            .into_iter()
            .map(|s| SessionSummary {
                session_id: s.session_id,
                model: s.model,
                provider: s.provider,
                message_count: s.messages.len(),
                updated_at: s.updated_at,
                created_at: s.created_at,
            })
            .collect(),
    )
}

/// Get full session detail including messages.
pub(crate) async fn get_session(
    Path(id): Path<String>,
) -> Result<Json<SessionDetail>, (StatusCode, String)> {
    let file = aa_session::storage::load_file(&id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            format!("Session '{id}' not found: {e}"),
        )
    })?;
    Ok(Json(SessionDetail {
        session_id: file.session_id,
        model: file.model,
        provider: file.provider,
        messages: file.messages,
        created_at: file.created_at,
        updated_at: file.updated_at,
    }))
}

/// Delete a session by ID.
#[utoipa::path(
    delete,
    path = "/sessions/{id}",
    responses(
        (status = 200, description = "Session deleted"),
        (status = 404, description = "Session not found")
    ),
    tag = "AAgent"
)]
pub(crate) async fn delete_session(
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let _ = aa_session::storage::delete(&id);
    Ok(Json(serde_json::json!({ "deleted": true })))
}
