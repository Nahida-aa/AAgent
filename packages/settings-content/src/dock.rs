//! Dock 位置枚举（对齐 zed `settings_content::DockPosition`）。
//!
//! ```rust
//! #[derive(Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, MergeFrom,
//!          PartialEq, Eq, strum::VariantArray, strum::VariantNames)]
//! #[serde(rename_all = "snake_case")]
//! pub enum DockPosition {
//!     Left, Bottom, Right,
//! }
//! ```
//!
//! 放在 settings-content 而不是 workspace，因为 settings JSON（default.json）
//! 需要序列化/反序列化它（例如面板的 `default_position` 字段）。

use serde::{Deserialize, Serialize};

/// Dock 在窗口中的位置。与 zed `DockPosition` 完全一致。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockPosition {
    Left,
    Bottom,
    Right,
}

impl DockPosition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Bottom => "Bottom",
            Self::Right => "Right",
        }
    }
}
