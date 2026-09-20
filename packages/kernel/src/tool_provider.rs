use std::sync::Arc;

/// 工具包——一组工具的集合。
///
/// 内核不关心工具的具体实现，只通过此 trait 按需获取工具列表。
/// 内置扩展、外部插件、WASM 模块等通过实现此 trait 向内核注册工具。
pub trait ToolProvider: Send + Sync {
    fn tools(&self, scope: &ToolProviderScope<'_>) -> Vec<Arc<dyn Tool>>;
}

/// 工具提供者的查询作用域，携带当前运行上下文。
#[derive(Debug, Clone)]
pub struct ToolProviderScope<'a> {
    pub working_dir: &'a str,
}

impl<'a> ToolProviderScope<'a> {
    pub fn new(working_dir: &'a str) -> Self { Self { working_dir } }
}

/// 工具的抽象接口。
///
/// 所有工具（无论是 Rust 原生、WASM 加载、子进程代理）都实现此 trait。
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    async fn execute(
        &self,
        arguments: serde_json::Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError>;
}

/// 工具元数据定义，用于 LLM 的 tool calling schema。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub origin: ToolOrigin,
    pub execution_mode: ExecutionMode,
}

/// 工具来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ToolOrigin {
    BuiltIn,
    Extension,
    Wasm,
    Subprocess,
}

/// 工具执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionMode {
    Sequential,
    Concurrent,
}

/// 工具执行上下文。
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub session_id: String,
    pub working_dir: String,
}

/// 工具执行结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl ToolResult {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            metadata: None,
        }
    }
}

/// 工具错误。
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    Execution(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Tool timed out after {0}ms")]
    Timeout(u64),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}
