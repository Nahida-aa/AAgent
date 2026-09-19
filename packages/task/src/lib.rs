//! AA Task — 任务定义与执行抽象。
//!
//! 对齐 zed `crates/task`：
//! - `SpawnInTerminal` — 启动终端任务所需的完整参数
//! - `RevealStrategy` / `HideStrategy` / `SaveStrategy` — task 展示/隐藏/保存策略
//! - `TaskHook` — 特殊 task action（非普通 shell 命令）

pub mod spawn_in_terminal;
pub mod task_template;

pub use spawn_in_terminal::{SpawnInTerminal, TaskId};
pub use task_template::{HideStrategy, RevealStrategy, SaveStrategy, TaskHook};
