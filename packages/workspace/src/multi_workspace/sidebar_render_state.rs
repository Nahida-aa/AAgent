//! Sidebar 状态类型 — SidebarStatus（entity 层）+ SidebarRenderState（render 层投影）。
//!
//! 对齐 zed `workspace::SidebarRenderState`（multi_workspace.rs L64）。
//!
//! 为什么两个类型？
//! - SidebarStatus — MultiWorkspace 持有的 Sidebar 开关/侧位置状态。
//!   **留在 workspace crate**，因为 MultiWorkspace / StatusBar 都需要读它，
//!   不能依赖 sidebar crate（会循环）。Sidebar entity 独立后是纯内容容器，
//!   不知道 open/side（那是 MultiWorkspace 的事）。
//! - SidebarRenderState — MultiWorkspace 投影的只读视图，暴露给 PlatformTitleBar 等上层。
//!   Zed 也保持两个独立类型 — 演进速度不同。

use settings_content::SidebarSide;

/// Sidebar 开关状态（entity 层）。
/// **留在 workspace crate** — Sidebar entity 独立后，workspace 不依赖 sidebar crate，
/// 所以 SidebarStatus 不能跟着 entity 走。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SidebarStatus {
    pub open: bool,
    pub side: SidebarSide,
}

impl Default for SidebarStatus {
    fn default() -> Self {
        Self {
            open: false,
            side: SidebarSide::Left,
        }
    }
}

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
