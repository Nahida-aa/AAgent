use std::sync::Arc;

use aa_core::llm::{Message, ModelProvider, Role};
use clap::Args;

#[derive(Args)]
pub struct RunArgs {
    #[arg(short, long, default_value = ".")]
    pub working_dir: String,
    #[arg(long)]
    pub no_llm: bool,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub base_url: Option<String>,
    #[arg(long)]
    pub provider: Option<String>,
    /// 会话 ID（"new" 新会话，"last" 用最近一个）
    #[arg(long, default_value = "new")]
    pub session: String,
}

pub async fn cmd_run(
    args: &RunArgs,
    _kernel: &aa_kernel::Kernel,
    registry: &aa_kernel::ToolRegistry,
) {
    let tool_defs = registry.all_definitions();
    let tool_instances = registry.all_tools();

    println!("AAgent ready | tools: {}", tool_defs.len());

    if args.no_llm {
        for t in &tool_defs {
            println!("  {} - {}", t.name, t.description);
        }
        return;
    }

    let cfg = settings_content::Config::load();
    let resolved = cfg.resolve(
        args.provider.as_deref(),
        args.model.as_deref(),
        args.base_url.as_deref(),
    );

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

    // ── Resolve session ID ─────────────────────────────────
    let session_id = match args.session.as_str() {
        "new" => uuid::Uuid::new_v4().to_string(),
        "last" => match aa_session::storage::list() {
            Ok(sessions) if !sessions.is_empty() => sessions[0].session_id.clone(),
            _ => {
                eprintln!("No saved sessions found. Starting new one.");
                uuid::Uuid::new_v4().to_string()
            }
        },
        id => id.to_string(),
    };

    // ── Load or init messages ───────────────────────────────
    let mut messages: Vec<Message> = if args.session != "new" {
        aa_session::storage::load(&session_id).unwrap_or_else(|_| vec![system_message()])
    } else {
        vec![system_message()]
    };

    if messages.len() == 1 {
        eprintln!("Session: {} (new)", &session_id[..8]);
    } else {
        let msg_count = messages.iter().filter(|m| m.role != Role::System).count();
        eprintln!(
            "Session: {} (continuing, {} messages)",
            &session_id[..8],
            msg_count
        );
    }

    println!("Type your message (Ctrl+C to exit)\n");

    // ── Print existing messages ─────────────────────────────
    for msg in &messages {
        match msg.role {
            Role::User => println!("\nYou: {}", msg.content),
            Role::Assistant => println!("\nAI: {}", msg.content),
            Role::Tool => {} // skip tool results in replay
            Role::System => {}
        }
    }

    loop {
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }
        let input = input.trim().to_owned();

        messages.push(Message {
            role: Role::User,
            content: input,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        let (tx, mut rx) = tokio::sync::mpsc::channel(64);

        let turn_input = aa_session::TurnInput {
            messages,
            provider: provider.clone(),
            tools: tool_instances.clone(),
            model: resolved.model.clone(),
            working_dir: args.working_dir.clone(),
            session_id: session_id.clone(),
        };

        let handle = tokio::spawn(aa_session::run_turn(turn_input, tx));

        while let Some(event) = rx.blocking_recv() {
            match event {
                aa_session::SessionEvent::Token(text) => {
                    print!("{text}");
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                aa_session::SessionEvent::ToolCall(tc) => {
                    println!("\n  \x1b[33m↻ tool: {}\x1b[0m", &tc.function.name);
                }
                aa_session::SessionEvent::ToolResult {
                    content, is_error, ..
                } => {
                    if is_error {
                        println!("  \x1b[31m✗ error:\x1b[0m {}", content);
                    } else {
                        // Truncate long results for display
                        let preview = if content.len() > 200 {
                            format!("{}...", &content[..200])
                        } else {
                            content.clone()
                        };
                        println!("  \x1b[32m✓ ok\x1b[0m: {}", preview);
                    }
                }
                aa_session::SessionEvent::Done { usage } => {
                    println!();
                    if let Some(u) = usage {
                        eprintln!(
                            "  tokens: {} prompt + {} completion = {} total",
                            u.prompt_tokens, u.completion_tokens, u.total_tokens
                        );
                    }
                }
                aa_session::SessionEvent::Error(msg) => {
                    eprintln!("\n\x1b[31mError:\x1b[0m {msg}");
                }
            }
        }

        let result = handle.await.expect("turn task panicked");
        messages = result.messages;

        // ── Persist after each turn ─────────────────────────
        if let Err(e) =
            aa_session::storage::save(&session_id, &messages, &resolved.model, &resolved.provider)
        {
            eprintln!("Warning: failed to save session: {e}");
        }
    }
}

fn system_message() -> Message {
    Message {
        role: Role::System,
        content: "You are AAgent, an AI assistant. You have access to filesystem tools. Help the user with their tasks.".into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }
}
