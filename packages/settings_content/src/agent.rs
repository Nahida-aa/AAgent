//! Agent 相关 settings — sidebar 位置等。
//!
//! 两个 enum 对齐 zed `crates/settings_content/src/agent.rs`:
//! - `SidebarDockPosition` — settings JSON 反序列化层（带 serde + JsonSchema）
//! - `SidebarSide` — 运行时层（简化的 Copy enum）
//!
//! Sidebar 是侧边栏概念，永远不会在 Bottom（那是 Dock 的位置）。

use serde::{Deserialize, Serialize};

/// Settings JSON 层 — 带 serde + JsonSchema。
/// 用于 `agent.sidebar_side` 字段的反序列化。
/// 对齐 zed `SidebarDockPosition`（agent.rs L31）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarDockPosition {
    /// Always show the sidebar on the left side.
    #[default]
    Left,
    /// Always show the sidebar on the right side.
    Right,
}

/// 运行时层 — 简化 Copy enum。
/// 对齐 zed `SidebarSide`（agent.rs L39）。
/// Sidebar entity / MultiWorkspace / StatusBar 内部都用这个。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarSide {
    #[default]
    Left,
    Right,
}

// 双向转换 — settings 层 ↔ 运行时层
impl From<SidebarDockPosition> for SidebarSide {
    fn from(p: SidebarDockPosition) -> Self {
        match p {
            SidebarDockPosition::Left => Self::Left,
            SidebarDockPosition::Right => Self::Right,
        }
    }
}

impl From<SidebarSide> for SidebarDockPosition {
    fn from(s: SidebarSide) -> Self {
        match s {
            SidebarSide::Left => Self::Left,
            SidebarSide::Right => Self::Right,
        }
    }
}

impl SidebarSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }
}
