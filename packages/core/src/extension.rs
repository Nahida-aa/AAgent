//! 扩展系统核心定义。
//!
//! 扩展是 AAgent 的主要扩展机制。内置 Rust 插件、WASM 插件、s5r 子进程插件
//! 都通过此系统接入内核。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::event::ExtensionEvent;

// ─── Extension Trait ─────────────────────────────────────────────────────

/// 扩展的核心接口。
///
/// 扩展通过实现此 trait 挂入 AAgent 生命周期。
/// register() 阶段声明能力（工具、钩子、命令），
/// start() 后进入运行态。
#[async_trait::async_trait]
pub trait Extension: Send + Sync {
    /// 扩展唯一标识符。
    fn id(&self) -> &str;

    /// 声明扩展需要宿主授予的能力。
    fn capabilities(&self) -> &[ExtensionCapability] {
        &[]
    }

    /// 一次性调用。扩展通过 registrar 注册工具、命令和事件处理器。
    fn register(&self, _reg: &mut Registrar) {}

    /// 扩展进入运行态。
    async fn start(&self, _ctx: ExtensionCtx) -> Result<(), ExtensionError> {
        Ok(())
    }

    /// 扩展退出运行态。
    async fn stop(&self, _reason: StopReason) -> Result<(), ExtensionError> {
        Ok(())
    }

    /// 健康检查。
    async fn health(&self) -> Result<(), ExtensionError> {
        Ok(())
    }

    /// 配置热更新。
    async fn on_config_changed(&self, _config: ExtensionConfig) -> Result<(), ExtensionError> {
        Ok(())
    }
}

// ─── Extension Capability ────────────────────────────────────────────────

/// 扩展可以显式申请的宿主能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCapability {
    /// 访问 session 下命名空间隔离的持久状态。
    SessionState,
    /// 创建子 session、提交 turn。
    SessionControl,
    /// 调用宿主配置的主模型。
    MainModel,
    /// 调用宿主配置的小模型。
    SmallModel,
    /// 只读查询历史 session。
    SessionHistory,
    /// 发射已声明的扩展事件。
    EmitEvents,
    /// 读取工作区或扩展发现目录。
    WorkspaceRead,
    /// 启动子进程。
    ProcessSpawn,
    /// 发起网络请求。
    NetworkClient,
}

// ─── Extension Config ────────────────────────────────────────────────────

/// 扩展专有配置包装。
#[derive(Clone, Debug, Default)]
pub struct ExtensionConfig(pub serde_json::Value);

impl ExtensionConfig {
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.0.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.0.as_object().is_some_and(|o| o.is_empty())
    }
}

// ─── Extension Context ───────────────────────────────────────────────────

/// 扩展运行态上下文。
#[derive(Clone)]
pub struct ExtensionCtx {
    pub config: ExtensionConfig,
    pub tasks: ExtensionTasks,
    pub working_dir: Option<String>,
}

impl ExtensionCtx {
    pub fn new(tasks: ExtensionTasks) -> Self {
        Self {
            tasks,
            config: ExtensionConfig::default(),
            working_dir: None,
        }
    }

    pub fn with_config(tasks: ExtensionTasks, config: ExtensionConfig) -> Self {
        Self {
            tasks,
            config,
            working_dir: None,
        }
    }

    pub fn shutdown(&self) -> tokio_util::sync::CancellationToken {
        self.tasks.shutdown()
    }
}

/// 扩展退出原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Reload,
    Disabled,
    Shutdown,
}

/// 宿主管理的扩展后台任务集合。
#[derive(Clone)]
pub struct ExtensionTasks {
    shutdown: tokio_util::sync::CancellationToken,
    handles: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl ExtensionTasks {
    pub fn new() -> Self {
        Self {
            shutdown: tokio_util::sync::CancellationToken::new(),
            handles: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn shutdown(&self) -> tokio_util::sync::CancellationToken {
        self.shutdown.clone()
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if self.shutdown.is_cancelled() {
            return;
        }
        let handle = tokio::spawn(fut);
        if let Ok(mut handles) = self.handles.lock() {
            handles.push(handle);
        }
    }

    pub fn cancel(&self) {
        self.shutdown.cancel();
    }

    pub async fn wait(&self, timeout: std::time::Duration) {
        let Ok(mut handles) = self.handles.lock() else {
            return;
        };
        let tasks = std::mem::take(&mut *handles);
        drop(handles);
        let deadline = tokio::time::Instant::now() + timeout;
        for handle in tasks {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                handle.abort();
            } else {
                let _ = tokio::time::timeout(deadline - now, handle).await;
            }
        }
    }
}

// ─── Registrar ───────────────────────────────────────────────────────────

/// 扩展能力注册器。
///
/// 在 Extension::register() 期间有效，扩展通过它声明自己提供的能力。
pub struct Registrar {
    tools: Vec<(crate::ToolDefinition, Arc<dyn ToolHandler>)>,
    hooks: Vec<(ExtensionEvent, HookMode, i32, Arc<dyn LifecycleHandler>)>,
    #[allow(dead_code)]
    capabilities: Vec<ExtensionCapability>,
}

impl Registrar {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            hooks: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    pub fn tool(&mut self, def: crate::ToolDefinition, handler: Arc<dyn ToolHandler>) {
        self.tools.push((def, handler));
    }

    pub fn on_event(
        &mut self,
        event: ExtensionEvent,
        mode: HookMode,
        priority: i32,
        handler: Arc<dyn LifecycleHandler>,
    ) {
        self.hooks.push((event, mode, priority, handler));
    }

    pub fn tools(&self) -> &[(crate::ToolDefinition, Arc<dyn ToolHandler>)] {
        &self.tools
    }

    pub fn hooks(&self) -> &[(ExtensionEvent, HookMode, i32, Arc<dyn LifecycleHandler>)] {
        &self.hooks
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.hooks.is_empty()
    }
}

impl Default for Registrar {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Handler Traits ──────────────────────────────────────────────────────

/// 工具执行处理器。
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
        working_dir: &str,
        ctx: &crate::ToolExecutionContext,
    ) -> Result<crate::ToolResult, ExtensionError>;
}

/// 通用生命周期钩子处理器。
#[async_trait::async_trait]
pub trait LifecycleHandler: Send + Sync {
    async fn handle(&self, ctx: LifecycleContext) -> Result<HookResult, ExtensionError>;
}

// ─── Hook Mode ───────────────────────────────────────────────────────────

/// 钩子订阅的执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookMode {
    /// 同步执行，可以阻止操作。
    Blocking,
    /// 异步执行（即发即弃）。
    NonBlocking,
    /// 执行但结果仅供参考。
    Advisory,
}

// ─── Hook Results ────────────────────────────────────────────────────────

/// 通用钩子结果。
#[derive(Debug, Clone)]
pub enum HookResult {
    Allow,
    Block { reason: String },
}

// ─── Lifecycle Context ───────────────────────────────────────────────────

/// 生命周期钩子上下文。
#[derive(Clone)]
pub struct LifecycleContext {
    pub session_id: String,
    pub working_dir: String,
}

// ─── Extension Error ─────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("Extension not found: {0}")]
    NotFound(String),
    #[error("Hook timed out after {0}ms")]
    Timeout(u64),
    #[error("Blocked by hook: {reason}")]
    Blocked { reason: String },
    #[error("Extension error: {0}")]
    Internal(String),
}
