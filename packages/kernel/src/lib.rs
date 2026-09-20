//! AAgent 微内核。
//!
//! 拥有运行时级的注册表和宿主组合契约。具体工具、扩展加载器、
//! 服务器、CLI 从此 crate 外部注册自身。

pub mod tool_provider;
pub mod tool_registry;

pub use tool_provider::{ToolProvider, ToolProviderScope};
pub use tool_registry::ToolRegistry;

use std::sync::Arc;

/// 可组合的内核表面，供宿主组装运行时能力。
#[derive(Clone, Default)]
pub struct Kernel {
    providers: Arc<[Arc<dyn ToolProvider>]>,
}

impl Kernel {
    pub fn builder() -> KernelBuilder { KernelBuilder::default() }

    pub fn tool_providers(&self) -> &[Arc<dyn ToolProvider>] { &self.providers }

    pub fn build_tool_registry(&self, scope: &ToolProviderScope<'_>) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for provider in self.tool_providers() {
            for tool in provider.tools(scope) {
                registry.register(tool);
            }
        }
        registry
    }
}

/// 可嵌入 AAgent 内核的构建器。
#[derive(Default)]
#[allow(dead_code)]
pub struct KernelBuilder {
    providers: Vec<Arc<dyn ToolProvider>>,
}

impl KernelBuilder {
    pub fn with_tool_provider(mut self, provider: Arc<dyn ToolProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn build(self) -> Kernel {
        Kernel {
            providers: Arc::from(self.providers),
        }
    }
}
