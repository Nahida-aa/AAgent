use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::MergeFrom;

/// Where to position the threads sidebar.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum SidebarDockPosition {
    /// Always show the sidebar on the left side.
    #[default]
    Left,
    /// Always show the sidebar on the right side.
    Right,
}
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
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
