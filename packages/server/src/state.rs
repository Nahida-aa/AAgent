use std::sync::Arc;

use tokio::sync::RwLock;

use crate::terminal::TerminalManager;

pub(crate) type Registry = Arc<RwLock<aa_kernel::ToolRegistry>>;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) registry: Registry,
    pub(crate) resolved: settings::ResolvedConfig,
    pub(crate) mcp_count: usize,
    pub(crate) terminal: TerminalManager,
}
