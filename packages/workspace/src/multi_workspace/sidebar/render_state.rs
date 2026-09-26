use super::*;

//! SidebarRenderState — MultiWorkspace 对外暴露的 sidebar 只读投影。
//!
//! 对齐 zed `workspace::SidebarRenderState`（multi_workspace.rs L61）。
//! 给 PlatformTitleBar 等上层用。
//!
//! StatusBar 内部有自己的私有 SidebarStatus（在 status_bar/sidebar_status.rs），
//! 每帧 query() 从 MultiWorkspace 查，不缓存。

use settings_content::SidebarSide;

/// Sidebar 的只读渲染状态 — MultiWorkspace.render() 和 PlatformTitleBar 用。
/// 对齐 zed `SidebarRenderState`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarRenderState {
    pub open: bool,
    pub side: SidebarSide,
}
