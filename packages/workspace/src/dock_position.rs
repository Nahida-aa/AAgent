//! Dock 位置枚举，对齐 zed `DockPosition`（L324-L328）。
//!
//! Zed 的 Dock 系统只有 **3 个 Dock 实例**（左/底/右），
//! 每个 Dock 里装多个 Panel（标签页式）。本 enum 标识 Dock 在窗口的哪一侧。

use gpui::Axis;

/// Dock 在窗口中的位置。与 zed `DockPosition` 完全一致。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

    /// 左侧/右侧 Dock 的面板横向排列；底部 Dock 纵向排列。
    pub fn axis(self) -> Axis {
        match self {
            Self::Bottom => Axis::Vertical,
            Self::Left | Self::Right => Axis::Horizontal,
        }
    }
}
