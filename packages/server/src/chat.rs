use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::extract::State;
use axum::http::{Response, header};
use axum::response::Json;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utoipa::ToSchema;

use aa_core::llm::{Message, ModelProvider, Role, ToolCall, ToolCallFunction};

use crate::AppState;

// ---------------------------------------------------------------------------
// AG-UI wire format types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, ToSchema)]
pub(crate) struct ChatMsg {
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) content: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) tool_calls: Option<Vec<ToolCallWire>>,
    #[serde(default)]
    pub(crate) tool_call_id: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) id: Option<String>,
}

#[derive(Deserialize, Clone, ToSchema)]
pub(crate) struct ToolCallWire {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) call_type: String,
    pub(crate) function: ToolCallFuncWire,
}

#[derive(Deserialize, Clone, ToSchema)]
pub(crate) struct ToolCallFuncWire {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Deserialize, Clone, ToSchema)]
#[allow(dead_code)]
pub(crate) struct ToolDef {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct AGUIChatRequest {
    #[serde(default)]
    pub(crate) thread_id: Option<String>,
    #[serde(default)]
    pub(crate) run_id: Option<String>,
    pub(crate) messages: Vec<ChatMsg>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) tools: Vec<ToolDef>,
}

// ---------------------------------------------------------------------------
// AG-UI SSE event types
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SseEvent {
    #[serde(rename_all = "camelCase")]
    RunStarted { thread_id: String, run_id: String },
    #[serde(rename_all = "camelCase")]
    TextMessageStart { message_id: String },
    #[serde(rename_all = "camelCase")]
    TextMessageContent { message_id: String, delta: String },
    #[serde(rename_all = "camelCase")]
    TextMessageEnd { message_id: String },
    #[serde(rename_all = "camelCase")]
    ToolCallStart {
        tool_call_id: String,
        tool_call_name: String,
    },
    #[serde(rename_all = "camelCase")]
    ToolCallArgs { tool_call_id: String, delta: String },
    #[serde(rename_all = "camelCase")]
    ToolCallEnd {
        tool_call_id: String,
        input: serde_json::Value,
        result: String,
    },
    #[serde(rename_all = "camelCase")]
    RunFinished {
        thread_id: String,
        run_id: String,
        finish_reason: String,
    },
    #[serde(rename_all = "camelCase")]
    RunError {
        thread_id: String,
        run_id: String,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Message Part types (for TUI PromptInfo)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextPart {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) message_id: String,
    pub(crate) r#type: String,
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) synthetic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ignored: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<PartTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePart {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) message_id: String,
    pub(crate) r#type: String,
    pub(crate) mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) filename: Option<String>,
    pub(crate) url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<FilePartSource>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePartSource {
    pub(crate) text: PartTextRange,
    pub(crate) r#type: String,
    pub(crate) path: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PartTextRange {
    pub(crate) value: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentPart {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) message_id: String,
    pub(crate) r#type: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<AgentPartSource>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentPartSource {
    pub(crate) value: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PartTime {
    pub(crate) start: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) end: Option<f64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_text(content: &Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn chat_msg_to_message(msg: &ChatMsg) -> Message {
    let role = match msg.role.as_str() {
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        "system" => Role::System,
        _ => Role::User,
    };

    let tool_calls = msg.tool_calls.as_ref().map(|tcs| {
        tcs.iter()
            .map(|tc| ToolCall {
                id: tc.id.clone(),
                call_type: tc.call_type.clone(),
                function: ToolCallFunction {
                    name: tc.function.name.clone(),
                    arguments: tc.function.arguments.clone(),
                },
            })
            .collect()
    });

    Message {
        role,
        content: extract_text(&msg.content),
        tool_calls,
        tool_call_id: msg.tool_call_id.clone(),
        name: msg.name.clone(),
    }
}

async fn send_json(tx: &mpsc::Sender<String>, value: &impl Serialize) {
    if let Ok(json) = serde_json::to_string(value) {
        let _ = tx.send(json).await;
    }
}

// ---------------------------------------------------------------------------
// SSE stream adapter
// ---------------------------------------------------------------------------

struct SseRx(mpsc::Receiver<String>);

impl Stream for SseRx {
    type Item = Result<String, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut().0.poll_recv(cx) {
            Poll::Ready(Some(json)) => Poll::Ready(Some(Ok(format!("data: {json}\n\n")))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// SSE chat endpoint
// ---------------------------------------------------------------------------

pub(crate) async fn chat_sse(
    State(state): State<AppState>,
    Json(req): Json<AGUIChatRequest>,
) -> Response<Body> {
    let (tx, rx) = mpsc::channel::<String>(64);
    let registry = state.registry.clone();
    let thread_id = req
        .thread_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let run_id = req
        .run_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let resolved = state.resolved.clone();

    tokio::spawn(async move {
        if resolved.provider != "ollama" && resolved.api_key.is_empty() {
            send_json(
                &tx,
                &SseEvent::RunError {
                    thread_id: thread_id.clone(),
                    run_id: run_id.clone(),
                    message: "AA_LLM_API_KEY not set".into(),
                },
            )
            .await;
            return;
        }

        let provider: Arc<dyn ModelProvider> = match resolved.provider.as_str() {
            "ollama" => Arc::new(aa_llm::OllamaProvider::new(aa_llm::OllamaConfig {
                base_url: resolved.base_url.clone(),
                default_model: resolved.model.clone(),
            })),
            _ => Arc::new(aa_llm::OpenAiCompatibleProvider::new(
                aa_llm::OpenAiConfig {
                    base_url: resolved.base_url.clone(),
                    api_key: resolved.api_key.clone(),
                    default_model: resolved.model.clone(),
                },
            )),
        };

        let mut messages: Vec<Message> = req.messages.iter().map(chat_msg_to_message).collect();

        // ── Load previous session messages ────────────────
        if uuid::Uuid::parse_str(&thread_id).is_ok() {
            if let Ok(previous) = aa_session::storage::load(&thread_id) {
                let user_msg = messages.clone();
                messages = previous;
                messages.extend(user_msg);
            }
        }

        let tools = registry.read().await.all_tools();

        let (session_tx, mut session_rx) = mpsc::channel::<aa_session::SessionEvent>(64);

        // --- RUN_STARTED ---
        send_json(
            &tx,
            &SseEvent::RunStarted {
                thread_id: thread_id.clone(),
                run_id: run_id.clone(),
            },
        )
        .await;

        // --- Spawn session turn ---
        let turn_input = aa_session::TurnInput {
            messages,
            provider,
            tools,
            model: resolved.model.clone(),
            working_dir: ".".into(),
            session_id: thread_id.clone(),
        };

        let turn_handle = tokio::spawn(aa_session::run_turn(turn_input, session_tx));

        // --- Convert SessionEvent → AG-UI SSE ---
        let mut in_message = false;
        let mut msg_counter = 0u32;
        let mut pending_tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(event) = session_rx.recv().await {
            match event {
                aa_session::SessionEvent::Token(text) => {
                    if !in_message {
                        msg_counter += 1;
                        send_json(
                            &tx,
                            &SseEvent::TextMessageStart {
                                message_id: format!("msg_{msg_counter}"),
                            },
                        )
                        .await;
                        in_message = true;
                    }
                    send_json(
                        &tx,
                        &SseEvent::TextMessageContent {
                            message_id: format!("msg_{msg_counter}"),
                            delta: text,
                        },
                    )
                    .await;
                }
                aa_session::SessionEvent::ToolCall(tc) => {
                    if in_message {
                        send_json(
                            &tx,
                            &SseEvent::TextMessageEnd {
                                message_id: format!("msg_{msg_counter}"),
                            },
                        )
                        .await;
                        in_message = false;
                    }

                    pending_tool_calls.push(tc.clone());

                    send_json(
                        &tx,
                        &SseEvent::ToolCallStart {
                            tool_call_id: tc.id.clone(),
                            tool_call_name: tc.function.name.clone(),
                        },
                    )
                    .await;

                    send_json(
                        &tx,
                        &SseEvent::ToolCallArgs {
                            tool_call_id: tc.id,
                            delta: tc.function.arguments,
                        },
                    )
                    .await;
                }
                aa_session::SessionEvent::ToolResult { content, .. } => {
                    if let Some(tc) = pending_tool_calls.first().cloned() {
                        pending_tool_calls.remove(0);

                        let parsed_args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(serde_json::Value::Null);

                        send_json(
                            &tx,
                            &SseEvent::ToolCallEnd {
                                tool_call_id: tc.id,
                                input: parsed_args,
                                result: content,
                            },
                        )
                        .await;
                    }
                }
                aa_session::SessionEvent::Done { .. } => {
                    if in_message {
                        send_json(
                            &tx,
                            &SseEvent::TextMessageEnd {
                                message_id: format!("msg_{msg_counter}"),
                            },
                        )
                        .await;
                    }

                    if let Ok(result) = turn_handle.await {
                        let model = &result.model;
                        let _ = aa_session::storage::save(
                            &thread_id,
                            &result.messages,
                            model,
                            &result.provider.id().0,
                        );
                    }

                    send_json(
                        &tx,
                        &SseEvent::RunFinished {
                            thread_id,
                            run_id,
                            finish_reason: "stop".into(),
                        },
                    )
                    .await;
                    break;
                }
                aa_session::SessionEvent::Error(msg) => {
                    if in_message {
                        send_json(
                            &tx,
                            &SseEvent::TextMessageEnd {
                                message_id: format!("msg_{msg_counter}"),
                            },
                        )
                        .await;
                    }
                    send_json(
                        &tx,
                        &SseEvent::RunError {
                            thread_id,
                            run_id,
                            message: msg,
                        },
                    )
                    .await;
                    break;
                }
            }
        }
    });

    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(SseRx(rx)))
        .unwrap()
}
