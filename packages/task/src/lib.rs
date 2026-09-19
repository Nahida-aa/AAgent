//! AA Task — 任务定义与执行抽象。
//!
//! 对齐 zed `crates/task`：
//! - `SpawnInTerminal` — 启动终端任务所需的完整参数
//! - 后续会有 TaskTemplate、ResolvedTask、TaskContext 等

pub mod spawn_in_terminal;

pub use spawn_in_terminal::{SpawnInTerminal, TaskId};
