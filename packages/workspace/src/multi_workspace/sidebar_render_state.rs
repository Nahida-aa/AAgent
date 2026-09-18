//! SidebarRenderState — MultiWorkspace 从 Sidebar entity 投影出的只读渲染状态。
//!
//! 对齐 zed `workspace::SidebarRenderState`（multi_workspace.rs L64）。
//!
//! 为什么不直接用 SidebarStatus？
//! - SidebarStatus 属于 Sidebar entity（sidebar/mod.rs）— 是 "entity 状态" 层
//! - SidebarRenderState 属于 MultiWorkspace — 是 "render 层" 的只读投影
//! - PlatformTitleBar / 其他上层需要读 sidebar 状态但不应该直接依赖 Sidebar entity
//!   （避免循环依赖 + 隔离 entity 内部细节）
//!
//! Zed 的 SidebarRenderState 只有 `{ open, side }` — 和 SidebarStatus 完全同构。
//! 但 Zed 也保持了两个独立类型 — 它们演进速度不同（SidebarStatus 可能加字段，
//! SidebarRenderState 只暴露渲染需要的最小集）。

use crate::sidebar::SidebarStatus;
use settings_content::SidebarSide;

/// Sidebar 的只读渲染状态 — MultiWorkspace.render() 和 PlatformTitleBar 用。
/// 对齐 zed `SidebarRenderState`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarRenderState {
    pub open: bool,
    pub side: SidebarSide,
}

impl From<SidebarStatus> for SidebarRenderState {
    fn from(status: SidebarStatus) -> Self {
        Self {
            open: status.open,
            side: status.side,
        }
    }
}
