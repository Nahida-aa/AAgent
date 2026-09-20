use std::sync::Arc;

use crate::tool_provider::{Tool, ToolDefinition};

/// 工具注册表——按名称索引所有已注册的工具。
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { Self { tools: Vec::new() } }

    pub fn register(&mut self, tool: Arc<dyn Tool>) { self.tools.push(tool); }

    pub fn find_definition(&self, name: &str) -> Option<ToolDefinition> {
        self.tools
            .iter()
            .find(|t| t.definition().name == name)
            .map(|t| t.definition())
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .iter()
            .find(|t| t.definition().name == name)
            .map(Arc::clone)
    }

    pub fn all_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    pub fn all_tools(&self) -> Vec<Arc<dyn Tool>> { self.tools.clone() }

    pub fn len(&self) -> usize { self.tools.len() }

    pub fn is_empty(&self) -> bool { self.tools.is_empty() }
}
