//! 统一 LLM 层(对齐 opencode `packages/llm` 设计)。
//!
//! - `protocols/`:线协议实现(openai-chat / ollama-chat)。
//! - `providers/`:provider 注册表与薄门面(openai-compatible + ollama + 厂商 profile)。
//!
//! 类型与 `ModelProvider` trait 定义在 `aa-core::llm`,此处只做实现层,
//! 避免与 `aa-core` 形成循环依赖。

pub mod protocols;
pub mod providers;

pub use protocols::ollama_chat::{OllamaConfig, OllamaProvider};
pub use protocols::openai_chat::{OpenAiConfig, OpenAiCompatibleProvider};
pub use providers::{Provider, ProviderKind, ProviderRegistry};