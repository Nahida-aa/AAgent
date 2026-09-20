//! WASM 插件加载器。
//!
//! 使用 wasmtime Component Model 加载 WASM 扩展组件。

use std::sync::Arc;

use aa_core::extension::{Extension, ExtensionCtx, ExtensionError, Registrar, ToolHandler};
use aa_kernel::tool_provider::{
    ExecutionMode, ToolDefinition, ToolExecutionContext, ToolOrigin, ToolResult,
};
use wasmtime::component::{Component, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

bindgen!();

struct WasmComponent {
    engine: Engine,
    component: Component,
    linker: Linker<()>,
}

/// WASM 插件加载器。
pub struct WasmExtensionLoader {
    engine: Engine,
}

impl WasmExtensionLoader {
    pub fn new() -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    pub fn load_bytes(&self, wasm_bytes: &[u8]) -> anyhow::Result<Box<dyn Extension>> {
        let component = Component::new(&self.engine, wasm_bytes)?;
        let linker = Linker::new(&self.engine);
        let wasm = Arc::new(WasmComponent {
            engine: self.engine.clone(),
            component,
            linker,
        });

        let (plugin_id, tools) = Self::collect_info(&wasm)?;

        Ok(Box::new(WasmExtension {
            wasm,
            plugin_id,
            tools,
        }))
    }

    fn collect_info(
        wasm: &WasmComponent,
    ) -> Result<(String, Vec<serde_json::Value>), wasmtime::Error> {
        let mut store = Store::new(&wasm.engine, ());
        let bindings = ExtensionWorld::instantiate(&mut store, &wasm.component, &wasm.linker)?;
        let plugin = bindings.aa_extension_plugin();
        let id = plugin.call_get_id(&mut store)?;
        let tools_json = plugin.call_register(&mut store)?;
        let tools: Vec<serde_json::Value> = serde_json::from_str(&tools_json).unwrap_or_default();
        Ok((id, tools))
    }
}

struct WasmExtension {
    wasm: Arc<WasmComponent>,
    plugin_id: String,
    tools: Vec<serde_json::Value>,
}

#[async_trait::async_trait]
impl Extension for WasmExtension {
    fn id(&self) -> &str { &self.plugin_id }

    fn register(&self, reg: &mut Registrar) {
        for tool_val in &self.tools {
            let name = tool_val["name"].as_str().unwrap_or("unknown").to_owned();
            let desc = tool_val["description"].as_str().unwrap_or("").to_owned();
            let params = tool_val["parameters"].clone();

            let def = ToolDefinition {
                name: name.clone(),
                description: desc,
                parameters: params,
                origin: ToolOrigin::Wasm,
                execution_mode: ExecutionMode::Concurrent,
            };

            let handler = WasmToolHandler {
                tool_name: name,
                wasm: self.wasm.clone(),
            };

            reg.tool(def, Arc::new(handler));
        }
    }

    async fn start(&self, _ctx: ExtensionCtx) -> Result<(), ExtensionError> { Ok(()) }

    async fn stop(&self, _reason: aa_core::extension::StopReason) -> Result<(), ExtensionError> {
        Ok(())
    }
}

struct WasmToolHandler {
    tool_name: String,
    wasm: Arc<WasmComponent>,
}

#[async_trait::async_trait]
impl ToolHandler for WasmToolHandler {
    async fn execute(
        &self,
        _tool_name: &str,
        arguments: serde_json::Value,
        _working_dir: &str,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ExtensionError> {
        let session = ctx.session_id.clone();
        let wd = ctx.working_dir.clone();
        let args = serde_json::to_string(&arguments).unwrap_or_default();
        let tool = self.tool_name.clone();
        let wasm = self.wasm.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let mut store = Store::new(&wasm.engine, ());
            let bindings = ExtensionWorld::instantiate(&mut store, &wasm.component, &wasm.linker)
                .map_err(|e| e.to_string())?;
            let plugin = bindings.aa_extension_plugin();
            plugin
                .call_execute(&mut store, &tool, &args, &session, &wd)
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| ExtensionError::Internal(e.to_string()))?
        .map_err(|e| ExtensionError::Internal(e))?;

        let parsed: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| ExtensionError::Internal(format!("WASM parse: {e}")))?;

        Ok(ToolResult {
            content: parsed["content"].as_str().unwrap_or("").to_owned(),
            is_error: parsed["is_error"].as_bool().unwrap_or(false),
            metadata: parsed.get("metadata").cloned(),
        })
    }
}
