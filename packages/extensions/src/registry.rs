//! 扩展注册表——按扩展 ID 管理所有已加载扩展。

use std::sync::Arc;

use aa_core::extension::{
    Extension, ExtensionCtx, ExtensionError, LifecycleHandler, Registrar, StopReason, ToolHandler,
};

/// 扩展注册表，管理扩展的完整生命周期。
pub struct ExtensionRegistry {
    entries: Vec<ExtensionEntry>,
}

struct ExtensionEntry {
    ext: Box<dyn Extension>,
    registrar: Registrar,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 注册一个扩展。内部调用 register() 收集其声明的能力。
    pub fn register(&mut self, ext: Box<dyn Extension>) {
        let mut registrar = Registrar::new();
        ext.register(&mut registrar);
        self.entries.push(ExtensionEntry { ext, registrar });
    }

    /// 启动所有扩展。
    pub async fn start_all(
        &self,
        ctx_fn: impl Fn(&str) -> ExtensionCtx,
    ) -> Result<(), ExtensionError> {
        for entry in &self.entries {
            let ctx = ctx_fn(entry.ext.id());
            if let Err(e) = entry.ext.start(ctx).await {
                tracing::warn!("Extension {} failed to start: {}", entry.ext.id(), e);
            }
        }
        Ok(())
    }

    /// 停止所有扩展。
    pub async fn stop_all(&self) {
        for entry in self.entries.iter().rev() {
            if let Err(e) = entry.ext.stop(StopReason::Shutdown).await {
                tracing::warn!("Extension {} failed to stop: {}", entry.ext.id(), e);
            }
        }
    }

    /// 获取扩展 ID 列表。
    pub fn ids(&self) -> Vec<&str> { self.entries.iter().map(|e| e.ext.id()).collect() }

    /// 获取指定扩展的引用。
    pub fn get_by_id(&self, id: &str) -> Option<&Box<dyn Extension>> {
        self.entries
            .iter()
            .find(|e| e.ext.id() == id)
            .map(|e| &e.ext)
    }

    /// 遍历所有扩展的工具处理器。
    pub fn all_tool_handlers(&self) -> Vec<(aa_core::ToolDefinition, Arc<dyn ToolHandler>)> {
        self.entries
            .iter()
            .flat_map(|e| {
                let ext_id = e.ext.id();
                e.registrar
                    .tools()
                    .iter()
                    .map(|(def, handler)| {
                        let mut def = def.clone();
                        def.name = format!("{}__{}", ext_id, def.name);
                        (def, Arc::clone(handler))
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// 遍历所有扩展的钩子处理器。
    pub fn all_hooks(
        &self,
    ) -> Vec<(
        aa_core::event::ExtensionEvent,
        aa_core::extension::HookMode,
        i32,
        Arc<dyn LifecycleHandler>,
    )> {
        self.entries
            .iter()
            .flat_map(|e| e.registrar.hooks().iter().cloned())
            .collect()
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self { Self::new() }
}
