//! Agent 相关 settings — SidebarSide 等。

use serde::{Deserialize, Serialize};

/// Sidebar 在左侧还是右侧（对齐 zed `settings_content::SidebarSide`）。
///
/// Sidebar 是侧边栏概念，不可能在 Bottom（Bottom 是 Dock 的位置）。
/// 用单独 enum 比 DockPosition 更精确 — DockPosition 有 Bottom variant 是死代码。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarSide {
    #[default]
    Left,
    Right,
}

impl SidebarSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }
}
