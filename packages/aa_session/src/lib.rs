use std::sync::Arc;

use aa_core::llm::*;
use aa_kernel::tool_provider::{Tool, ToolExecutionContext};

pub mod storage;

// ── Events ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolResult {
        name: String,
        content: String,
        is_error: bool,
    },
    Done {
        usage: Option<Usage>,
    },
    Error(String),
}

// ── Turn ─────────────────────────────────────────────────────

pub struct TurnInput {
    pub messages: Vec<Message>,
    pub provider: Arc<dyn ModelProvider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub model: String,
    pub working_dir: String,
    pub session_id: String,
}

pub async fn run_turn(input: TurnInput, tx: tokio::sync::mpsc::Sender<SessionEvent>) -> TurnInput {
    let TurnInput {
        mut messages,
        provider,
        tools,
        model,
        working_dir,
        session_id,
    } = input;

    let tool_defs: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| serde_json::to_value(t.definition()).unwrap())
        .collect();

    loop {
        let request = ModelRequest {
            messages: messages.clone(),
            tools: tool_defs.clone(),
            config: ModelConfig {
                provider: provider.id(),
                model: model.clone(),
                temperature: None,
                max_tokens: Some(4096),
                top_p: None,
            },
        };

        let mut stream_rx = match provider.chat_stream(request).await {
            Ok(rx) => rx,
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(format!("{e}"))).await;
                return TurnInput {
                    messages,
                    provider,
                    tools,
                    model,
                    working_dir,
                    session_id,
                };
            }
        };

        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(event) = stream_rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    assistant_text.push_str(&text);
                    let _ = tx.send(SessionEvent::Token(text)).await;
                }
                StreamEvent::ToolCall(tc) => {
                    let _ = tx.send(SessionEvent::ToolCall(tc.clone())).await;
                    tool_calls.push(tc);
                }
                StreamEvent::Done(usage) => {
                    messages.push(Message {
                        role: Role::Assistant,
                        content: assistant_text.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });

                    if tool_calls.is_empty() {
                        let _ = tx.send(SessionEvent::Done { usage: Some(usage) }).await;
                        return TurnInput {
                            messages,
                            provider,
                            tools,
                            model,
                            working_dir,
                            session_id,
                        };
                    }

                    for tc in &tool_calls {
                        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null);
                        let ctx = ToolExecutionContext {
                            session_id: session_id.clone(),
                            working_dir: working_dir.clone(),
                        };

                        match tools
                            .iter()
                            .find(|t| t.definition().name == tc.function.name)
                        {
                            Some(tool) => match tool.execute(args, &ctx).await {
                                Ok(result) => {
                                    let content = result.content;
                                    messages.push(Message {
                                        role: Role::Tool,
                                        content: content.clone(),
                                        tool_calls: None,
                                        tool_call_id: Some(tc.id.clone()),
                                        name: Some(tc.function.name.clone()),
                                    });
                                    let _ = tx
                                        .send(SessionEvent::ToolResult {
                                            name: tc.function.name.clone(),
                                            content,
                                            is_error: result.is_error,
                                        })
                                        .await;
                                }
                                Err(e) => {
                                    let err = format!("Error: {e}");
                                    messages.push(Message {
                                        role: Role::Tool,
                                        content: err.clone(),
                                        tool_calls: None,
                                        tool_call_id: Some(tc.id.clone()),
                                        name: Some(tc.function.name.clone()),
                                    });
                                    let _ = tx
                                        .send(SessionEvent::ToolResult {
                                            name: tc.function.name.clone(),
                                            content: err,
                                            is_error: true,
                                        })
                                        .await;
                                }
                            },
                            None => {
                                let err = format!("Tool '{}' not found", tc.function.name);
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: err.clone(),
                                    tool_calls: None,
                                    tool_call_id: Some(tc.id.clone()),
                                    name: Some(tc.function.name.clone()),
                                });
                                let _ = tx
                                    .send(SessionEvent::ToolResult {
                                        name: tc.function.name.clone(),
                                        content: err,
                                        is_error: true,
                                    })
                                    .await;
                            }
                        }
                    }
                    tool_calls.clear();
                }
                StreamEvent::Error(msg) => {
                    let _ = tx.send(SessionEvent::Error(msg)).await;
                    return TurnInput {
                        messages,
                        provider,
                        tools,
                        model,
                        working_dir,
                        session_id,
                    };
                }
            }
        }
    }
}
