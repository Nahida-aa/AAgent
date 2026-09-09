//! AAgent 核心类型。
//!
//! 定义了扩展系统（Extension trait、Registrar、生命周期事件）、
//! LLM 抽象和存储接口。

pub mod event;
pub mod extension;
pub mod llm;

// 重新导出内核的工具类型，方便外部只依赖 aa-core。
pub use aa_kernel::tool_provider::{
    ExecutionMode, Tool, ToolDefinition, ToolError, ToolExecutionContext, ToolOrigin, ToolResult,
};
