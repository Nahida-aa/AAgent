//! Panel 定义 + PanelKind 枚举 + PanelEntry。

use ui_gpui::IconName;

use crate::dock_position::DockPosition;

// ---------- PanelKind ----------

/// 面板分类。每个 variant 知道自己的 icon/tooltip/default_position。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Project,
    Git,
    Collab,
    Outline,
    Terminal,
    Debug,
    Agent,
}

impl PanelKind {
    pub fn persistent_name(self) -> &'static str {
        match self {
            PanelKind::Project => "project",
            PanelKind::Git => "git",
            PanelKind::Collab => "collab",
            PanelKind::Outline => "outline",
            PanelKind::Terminal => "terminal",
            PanelKind::Debug => "debug",
            PanelKind::Agent => "agent",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            PanelKind::Project => IconName::FileTree,
            PanelKind::Git => IconName::GitBranch,
            PanelKind::Collab => IconName::UserGroup,
            PanelKind::Outline => IconName::ListTree,
            PanelKind::Terminal => IconName::TerminalAlt,
            PanelKind::Debug => IconName::Debug,
            PanelKind::Agent => IconName::ZedAssistant,
        }
    }

    pub fn aria_label(self) -> &'static str {
        match self {
            PanelKind::Project => "Project Panel",
            PanelKind::Git => "Git Panel",
            PanelKind::Collab => "Collab Panel",
            PanelKind::Outline => "Outline Panel",
            PanelKind::Terminal => "Terminal Panel",
            PanelKind::Debug => "Debug Panel",
            PanelKind::Agent => "Agent Panel",
        }
    }

    /// 默认 dock 位置，对齐 zed Classic 布局。
    pub fn default_position(self) -> DockPosition {
        match self {
            PanelKind::Project | PanelKind::Git => DockPosition::Left,
            PanelKind::Terminal => DockPosition::Bottom,
            _ => DockPosition::Right,
        }
    }
}

// ---------- PanelEntry ----------

/// Dock 里的一个面板条目（对齐 zed `dock.rs` 的 `PanelEntry` struct）。
#[derive(Clone, Copy)]
pub struct PanelEntry {
    pub kind: PanelKind,
}

impl PanelEntry {
    pub fn new(kind: PanelKind) -> Self {
        Self { kind }
    }

    pub fn persistent_name(&self) -> &'static str {
        self.kind.persistent_name()
    }

    pub fn icon(&self) -> IconName {
        self.kind.icon()
    }

    pub fn icon_tooltip(&self) -> &'static str {
        self.kind.aria_label()
    }

    pub fn default_position(&self) -> DockPosition {
        self.kind.default_position()
    }
}
