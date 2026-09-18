use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};
use tokio::sync::RwLock;

use crate::{AppState, Registry};

fn build_app(
    registry: Registry,
    resolved: settings_content::ResolvedConfig,
    mcp_count: usize,
    terminal: super::terminal::TerminalManager,
) -> Router {
    let state = AppState {
        registry,
        resolved,
        mcp_count,
        terminal,
    };

    Router::new()
        .route("/health", get(super::health::health))
        .route("/tools", get(super::tools::list_tools))
        .route("/tools/{name}", post(super::tools::call_tool))
        .route("/chat", post(super::chat::chat_sse))
        .route("/sessions", get(super::sessions::list_sessions))
        .route(
            "/sessions/{id}",
            get(super::sessions::get_session).delete(super::sessions::delete_session),
        )
        .route(
            "/terminals",
            get(super::terminal::list_sessions).post(super::terminal::create_session),
        )
        .route("/terminals/{id}", delete(super::terminal::delete_session))
        .route("/terminal", get(super::terminal::ws_handler))
        .route("/terminal/{id}", get(super::terminal::ws_session_handler))
        .route("/openapi.json", get(super::api_doc::openapi_json))
        .with_state(state)
}

/// Build the kernel with built-in tool providers and optional MCP extensions.
fn build_kernel(config: &settings_content::Config) -> aa_kernel::Kernel {
    let mut builder = aa_kernel::Kernel::builder()
        .with_tool_provider(std::sync::Arc::new(aa_function_tools::FsToolProvider));

    if let Some(mcp_json) = config.mcp_servers_json() {
        builder = builder.with_tool_provider(std::sync::Arc::new(
            aa_extension_mcp::McpToolProvider::from_json(mcp_json),
        ));
    }

    builder.build()
}

/// Build the kernel, registry, config, and axum app, then bind to a port.
///
/// Returns the bound port, the TcpListener, and the Router.
/// The caller can either `axum::serve` on the listener (blocking) or
/// spawn it on a background task.
pub async fn build(
    port: u16,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_base_url: Option<&str>,
) -> anyhow::Result<(u16, tokio::net::TcpListener, Router)> {
    let config = settings_content::Config::load();
    let mcp_count = config
        .mcp_servers_json()
        .and_then(|j| j.as_array().map(|a| a.len()))
        .unwrap_or(0);
    let kernel = build_kernel(&config);
    let scope = aa_kernel::ToolProviderScope::new(".");
    let registry = Arc::new(RwLock::new(kernel.build_tool_registry(&scope)));

    let resolved = config.resolve(cli_provider, cli_model, cli_base_url);
    let terminal = super::terminal::TerminalManager::new();

    let app = build_app(registry, resolved, mcp_count, terminal);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    let actual_port = listener.local_addr().expect("local_addr").port();
    tracing::info!("aaServer listening on http://0.0.0.0:{actual_port}");

    Ok((actual_port, listener, app))
}

/// Start the aa server on the given port and block forever.
///
/// When `enable_mdns` is true, registers an mDNS service (`_aa._tcp.local`)
/// for LAN discovery.
///
/// Optional config overrides (provider, model, base_url) take highest priority.
pub async fn serve(
    port: u16,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_base_url: Option<&str>,
    enable_mdns: bool,
) -> anyhow::Result<()> {
    let (actual_port, listener, app) = build(port, cli_provider, cli_model, cli_base_url).await?;

    let _mdns = if enable_mdns {
        match super::mdns::register(actual_port) {
            Ok(d) => {
                tracing::info!("mDNS: _aa._tcp.local registered");
                Some(d)
            }
            Err(e) => {
                tracing::warn!("mDNS: failed to register: {e}");
                None
            }
        }
    } else {
        None
    };

    axum::serve(listener, app).await?;
    Ok(())
}
