use aa_core::extension::{Extension, Registrar};
use aa_extensions::wasm::WasmExtensionLoader;

/// 测试 WASM 插件加载和执行全流程。
#[test]
fn test_wasm_plugin_load_and_execute() {
    let wasm_bytes = wat::parse_str(test_wasm_component()).expect("WAT parsing failed");

    let loader = WasmExtensionLoader::new().expect("WasmExtensionLoader creation failed");

    let ext = loader
        .load_bytes(&wasm_bytes)
        .expect("WASM component loading failed");

    assert_eq!(ext.id(), "hello-wasm");

    let mut reg = Registrar::new();
    ext.register(&mut reg);
    let tools = reg.tools();
    assert_eq!(tools.len(), 1, "should register 1 tool");

    let (def, handler) = &tools[0];
    assert_eq!(def.name, "hello");
    assert_eq!(def.description, "Say hello");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        let ctx = aa_kernel::tool_provider::ToolExecutionContext {
            session_id: "test-session".into(),
            working_dir: "/tmp".into(),
        };
        handler
            .execute("hello", serde_json::json!({"name": "world"}), "/tmp", &ctx)
            .await
    });

    let result = result.expect("tool execution should succeed");
    assert!(!result.is_error, "result should not be an error");
    assert_eq!(result.content, "Hello from WASM!");
}

/// 测试 WAT 组件源码，用于内联测试。
fn test_wasm_component() -> &'static str { include_str!("test_wasm_plugin.wat") }
