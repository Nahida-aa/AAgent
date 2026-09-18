use std::sync::Arc;

use clap::{Parser, Subcommand};

mod clipboard;
mod markdown;
mod run;

#[derive(Parser)]
#[command(name = "aa", version, about = "AAgent - 个人 AI 助手")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// LLM provider
    #[arg(long, global = true)]
    pub provider: Option<String>,
    /// Model name
    #[arg(long, global = true)]
    pub model: Option<String>,
    /// API base URL
    #[arg(long, global = true)]
    pub base_url: Option<String>,
    /// Working directory
    #[arg(long, global = true)]
    pub working_dir: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// 行模式对话
    Run(run::RunArgs),
    /// 启动 HTTP 服务
    Serve {
        /// 监听端口（默认 3000）
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// 启用 mDNS 局域网发现（默认不启用）
        #[arg(long)]
        mdns: bool,
    },
    /// 连接到远程 server
    Attach {
        /// 远程 server URL（如 http://192.168.1.100:3000）
        url: String,
    },
    /// 工具管理
    Tool(ToolArgs),
    /// 扩展管理
    Extension(ExtensionArgs),
    /// 会话管理
    Session(SessionArgs),
}

#[derive(clap::Args)]
struct ToolArgs {
    #[command(subcommand)]
    command: ToolCommand,
}

#[derive(Subcommand)]
enum ToolCommand {
    List,
    Call { name: String, arguments: String },
}

#[derive(clap::Args)]
struct ExtensionArgs {
    #[command(subcommand)]
    command: ExtensionCommand,
}

#[derive(Subcommand)]
enum ExtensionCommand {
    List,
}

#[derive(clap::Args)]
struct SessionArgs {
    #[command(subcommand)]
    command: SessionCommand,
}

#[derive(Subcommand)]
enum SessionCommand {
    /// 列出所有会话
    List,
    /// 删除会话
    Delete { session_id: String },
}

fn main() {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let working_dir = args.working_dir.as_deref().unwrap_or(".");

    match args.command {
        None => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            let exit_code = rt.block_on(async {
                let (port, listener, app) = aa_server::build(0, None, None, None)
                    .await
                    .expect("server build");
                let url = format!("http://localhost:{port}");

                tracing::info!("Starting TUI, server at {url}");

                let server_task = tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("Server error: {e}");
                    }
                });

                let code = spawn_tui(Some(&url), working_dir);

                server_task.abort();
                code
            });
            std::process::exit(exit_code.unwrap_or(1));
        }
        Some(Command::Run(ref run_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(async {
                let cfg = settings::Config::load();
                let kernel = build_kernel(&cfg);
                let registry = build_registry(&kernel, working_dir);
                run::cmd_run(run_args, &kernel, &registry).await;
            });
        }
        Some(Command::Serve { port, mdns }) => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(async {
                aa_server::serve(
                    port,
                    args.provider.as_deref(),
                    args.model.as_deref(),
                    args.base_url.as_deref(),
                    mdns,
                )
                .await
                .expect("server failed");
            });
        }
        Some(Command::Attach { ref url }) => {
            spawn_tui(Some(url), working_dir);
            std::process::exit(0);
        }
        Some(Command::Tool(ref tool_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(cmd_tool(tool_args));
        }
        Some(Command::Extension(ref _ext_args)) => {
            println!("No extensions loaded (built-in only)");
        }
        Some(Command::Session(ref session_args)) => match &session_args.command {
            SessionCommand::List => match aa_session::storage::list() {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No saved sessions");
                    } else {
                        println!("Sessions:");
                        for s in &sessions {
                            println!(
                                "  {}  (model: {}, {} msgs)",
                                &s.session_id[..8],
                                s.model,
                                s.messages.len()
                            );
                        }
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            },
            SessionCommand::Delete { session_id } => {
                aa_session::storage::delete(session_id).ok();
                println!("Deleted session {session_id}");
            }
        },
    }
}

fn spawn_tui(attach_url: Option<&str>, _working_dir: &str) -> Option<i32> {
    let mut cmd = std::process::Command::new("bun");
    cmd.args(["run", "--conditions=browser", "packages/tui/src/index.tsx"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    if let Some(url) = attach_url {
        cmd.env("AA_SERVER_URL", url);
    }

    match cmd.spawn().and_then(|mut child| child.wait()) {
        Ok(exit) => Some(exit.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("Failed to launch TUI: {e}");
            eprintln!("Make sure bun is installed and the TUI dependencies are set up:");
            eprintln!("  cd packages/tui && bun install");
            None
        }
    }
}

async fn cmd_tool(args: &ToolArgs) {
    let cfg = settings::Config::load();
    let kernel = build_kernel(&cfg);
    let registry = build_registry(&kernel, ".");

    match &args.command {
        ToolCommand::List => {
            for t in registry.all_definitions() {
                println!("{}: {}", t.name, t.description);
            }
        }
        ToolCommand::Call { name, arguments } => {
            let args: serde_json::Value =
                serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
            let ctx = aa_kernel::tool_provider::ToolExecutionContext {
                session_id: "cli".into(),
                working_dir: ".".into(),
            };

            match registry.find(name) {
                Some(tool) => match tool.execute(args, &ctx).await {
                    Ok(result) => println!("{}", result.content),
                    Err(e) => eprintln!("Error: {e}"),
                },
                None => eprintln!("Tool '{name}' not found"),
            }
        }
    }
}

pub(crate) fn build_kernel(config: &settings::Config) -> aa_kernel::Kernel {
    let mut builder = aa_kernel::Kernel::builder()
        .with_tool_provider(Arc::new(aa_function_tools::FsToolProvider));

    if let Some(mcp_json) = config.mcp_servers_json() {
        builder = builder.with_tool_provider(Arc::new(
            aa_extension_mcp::McpToolProvider::from_json(mcp_json),
        ));
    }

    builder.build()
}

pub(crate) fn build_registry(
    kernel: &aa_kernel::Kernel,
    working_dir: &str,
) -> aa_kernel::ToolRegistry {
    let scope = aa_kernel::ToolProviderScope::new(working_dir);
    kernel.build_tool_registry(&scope)
}
