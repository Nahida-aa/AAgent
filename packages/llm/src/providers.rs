//! Provider 层:注册表 + 薄门面(对齐 opencode `providers/` 概念)。
//!
//! - `ProviderKind` 描述一个可构建的 provider(openai-compatible / ollama)。
//! - `ProviderRegistry` 按 id 存 `Box<dyn ModelProvider>`,供运行时按名取用。
//! - opencode 式 `Provider::configure(kind)` 让"厂商配置"先于"模型选择"。

use std::collections::HashMap;
use std::sync::Arc;

use aa_core::llm::ModelProvider;

use crate::protocols::ollama_chat::{OllamaConfig, OllamaProvider};
use crate::protocols::openai_chat::{OpenAiCompatibleProvider, OpenAiConfig};

/// 可构建的 provider 种类。
#[derive(Debug, Clone)]
pub enum ProviderKind {
    OpenAiCompatible(OpenAiConfig),
    Ollama(OllamaConfig),
}

impl ProviderKind {
    /// provider 稳定标识(对应 `ModelProvider::id().0`)。
    pub fn kind_id(&self) -> &'static str {
        match self {
            Self::OpenAiCompatible(_) => "openai-compatible",
            Self::Ollama(_) => "ollama",
        }
    }

    /// 构建具体的 provider 实例。
    pub fn build(&self) -> Box<dyn ModelProvider> {
        match self {
            Self::OpenAiCompatible(config) => {
                Box::new(OpenAiCompatibleProvider::new(config.clone()))
            }
            Self::Ollama(config) => Box::new(OllamaProvider::new(config.clone())),
        }
    }

    /// 构建可分享给多任务的 provider 实例。
    pub fn build_arc(&self) -> Arc<dyn ModelProvider> { Arc::from(self.build()) }

    /// 本地 Ollama。
    pub fn ollama(base_url: impl Into<String>, default_model: impl Into<String>) -> Self {
        Self::Ollama(OllamaConfig {
            base_url: base_url.into(),
            default_model: default_model.into(),
        })
    }

    /// 通用 OpenAI 兼容端点。
    pub fn openai_compatible(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self::OpenAiCompatible(OpenAiConfig {
            base_url: base_url.into(),
            api_key: api_key.into(),
            default_model: default_model.into(),
        })
    }
}

/// opencode 式薄门面:`configure` 决定厂商,`model` 选择模型。
#[derive(Debug, Clone)]
pub struct Provider {
    pub id: String,
    pub kind: ProviderKind,
}

impl Provider {
    /// 先定厂商再做模型选择。
    pub fn configure(kind: ProviderKind) -> Self {
        Self {
            id: kind.kind_id().to_owned(),
            kind,
        }
    }

    pub fn build(&self) -> Box<dyn ModelProvider> { self.kind.build() }
}

/// 内置厂商 profile(都走 openai-compatible 协议)。
pub mod profiles {
    use super::ProviderKind;

    /// DeepSeek。
    pub fn deepseek(api_key: impl Into<String>, default_model: impl Into<String>) -> ProviderKind {
        ProviderKind::openai_compatible("https://api.deepseek.com/v1", api_key, default_model)
    }

    /// Groq(免费高速推理)。
    pub fn groq(api_key: impl Into<String>, default_model: impl Into<String>) -> ProviderKind {
        ProviderKind::openai_compatible("https://api.groq.com/openai/v1", api_key, default_model)
    }

    /// Cerebras(高速推理)。
    pub fn cerebras(api_key: impl Into<String>, default_model: impl Into<String>) -> ProviderKind {
        ProviderKind::openai_compatible("https://api.cerebras.ai/v1", api_key, default_model)
    }

    /// Together AI。
    pub fn togetherai(
        api_key: impl Into<String>,
        default_model: impl Into<String>,
    ) -> ProviderKind {
        ProviderKind::openai_compatible("https://api.together.xyz/v1", api_key, default_model)
    }
}

/// 按 `ProviderId` 注册的 provider 集合。
#[derive(Default)]
pub struct ProviderRegistry {
    map: HashMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self { Self::default() }

    /// 注册一种 provider,id 取自 `ProviderKind::kind_id()`。
    pub fn register(&mut self, kind: ProviderKind) -> &mut Self {
        self.register_as(kind.kind_id(), kind)
    }

    /// 以自定义 id 注册。
    pub fn register_as(&mut self, id: impl Into<String>, kind: ProviderKind) -> &mut Self {
        self.map.insert(id.into(), kind.build_arc());
        self
    }

    /// 借出 provider 引用。
    pub fn get(&self, id: &str) -> Option<&dyn ModelProvider> {
        self.map.get(id).map(|a| a.as_ref())
    }

    /// 取出可分享的 provider 实例。
    pub fn get_arc(&self, id: &str) -> Option<Arc<dyn ModelProvider>> { self.map.get(id).cloned() }

    pub fn ids(&self) -> impl Iterator<Item = &str> { self.map.keys().map(|s| s.as_str()) }
}
