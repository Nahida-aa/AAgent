//! 主机路由器——在不可信扩展调用时执行安全沙箱。

use std::sync::Arc;

use aa_core::extension::{ExtensionCapability, ExtensionError, ToolHandler};
use parking_lot::RwLock;

/// 主机路由器决定扩展的每个工具调用是否放行。
pub struct HostRouter {
    grants: RwLock<Vec<(String, Vec<ExtensionCapability>)>>,
    handler_map: RwLock<Vec<(String, Arc<dyn ToolHandler>, Vec<ExtensionCapability>)>>,
}

impl HostRouter {
    pub fn new() -> Self {
        Self {
            grants: RwLock::new(Vec::new()),
            handler_map: RwLock::new(Vec::new()),
        }
    }

    /// 注册一个扩展的工具处理程序及宿主授予的能力。
    pub fn register_tools(
        &self,
        extension_id: &str,
        tools: Vec<(aa_core::ToolDefinition, Arc<dyn ToolHandler>)>,
        capabilities: &[ExtensionCapability],
    ) {
        let mut grants = self.grants.write();
        grants.push((extension_id.to_owned(), capabilities.to_vec()));
        let mut map = self.handler_map.write();
        for (def, handler) in tools {
            let prefixed_name = format!("{extension_id}__{}", def.name);
            map.push((prefixed_name, handler, capabilities.to_vec()));
        }
    }

    /// 解析扩展名（去掉前缀），如果调用无权限则拒绝。
    pub async fn call_tool(
        &self,
        prefixed_name: &str,
        args: serde_json::Value,
        working_dir: &str,
        ctx: &aa_core::ToolExecutionContext,
    ) -> Result<aa_core::ToolResult, ExtensionError> {
        let handler = {
            let map = self.handler_map.read();
            map.iter()
                .find(|(name, _, _)| name == prefixed_name)
                .map(|(_, handler, _)| Arc::clone(handler))
        };
        match handler {
            Some(handler) => handler.execute(prefixed_name, args, working_dir, ctx).await,
            None => Err(ExtensionError::NotFound(format!(
                "Tool {prefixed_name} not found in any extension"
            ))),
        }
    }
}

impl Default for HostRouter {
    fn default() -> Self { Self::new() }
}
