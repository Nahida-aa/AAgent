use super::*;

use gpui::Axis;
use settings::{DockPosition as SettingsDockPosition, TerminalDockPosition};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DockPosition {
    Left,
    Bottom,
    Right,
}

impl From<SettingsDockPosition> for DockPosition {
    fn from(value: SettingsDockPosition) -> Self {
        match value {
            SettingsDockPosition::Left => Self::Left,
            SettingsDockPosition::Bottom => Self::Bottom,
            SettingsDockPosition::Right => Self::Right,
        }
    }
}

impl Into<SettingsDockPosition> for DockPosition {
    fn into(self) -> SettingsDockPosition {
        match self {
            Self::Left => SettingsDockPosition::Left,
            Self::Bottom => SettingsDockPosition::Bottom,
            Self::Right => SettingsDockPosition::Right,
        }
    }
}

impl From<TerminalDockPosition> for DockPosition {
    fn from(value: TerminalDockPosition) -> Self {
        match value {
            TerminalDockPosition::Left => DockPosition::Left,
            TerminalDockPosition::Bottom => DockPosition::Bottom,
            TerminalDockPosition::Right => DockPosition::Right,
        }
    }
}

impl DockPosition {
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Bottom => "Bottom",
            Self::Right => "Right",
        }
    }

    pub fn axis(&self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Bottom => Axis::Vertical,
        }
    }
}
