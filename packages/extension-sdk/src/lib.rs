//! AAgent Extension SDK。
//!
//! 扩展开发的稳定公共 API。扩展作者只需依赖此 crate。
//! 所有需要暴露给扩展的类型均在此重新导出。

// ─── 核心扩展类型 ────────────────────────────────────────────────────
pub use aa_core::extension::{
    Extension, ExtensionCapability, ExtensionConfig, ExtensionCtx, ExtensionError, ExtensionTasks,
    HookMode, HookResult, LifecycleContext, LifecycleHandler, Registrar, StopReason, ToolHandler,
};

// ─── 工具类型 ─────────────────────────────────────────────────────────
pub use aa_kernel::tool_provider::{
    ExecutionMode, Tool, ToolDefinition, ToolError, ToolExecutionContext, ToolOrigin, ToolResult,
};

// ─── LLM 类型 ─────────────────────────────────────────────────────────
pub use aa_core::llm::{
    Message, ModelConfig, ModelError, ModelProvider, ModelRequest, ModelResponse, ModelResult,
    ProviderId, Role, StreamEvent, ToolCall, Usage,
};

// ─── 事件 ─────────────────────────────────────────────────────────────
pub use aa_core::event::ExtensionEvent;

// ─── 辅助宏 ───────────────────────────────────────────────────────────
/// 快速创建一个 ToolResult::text。
pub fn text_result(content: impl Into<String>) -> ToolResult {
    ToolResult::text(content)
}

/// 快速创建一个 ToolResult::error。
pub fn error_result(content: impl Into<String>) -> ToolResult {
    ToolResult::error(content)
}
