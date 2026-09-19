//! Project crate — 打开项目、管理 worktrees、per-project settings。
//!
//! 对齐 Zed `crates/project`，但做**纯数据骨架**（无 GPUI / LSP / collab 依赖）。
//! 完整的 Project runtime 等有 GPUI 绑定了再升级成 Entity。

pub mod entity;
pub mod fs;

pub use entity::{
    OpenProjectOptions, OpenWorktreeStrategy, Project, ProjectPath, WorktreeEntry, WorktreeId,
};
pub use fs::{Fs, Metadata, MockFs, RealFs};
