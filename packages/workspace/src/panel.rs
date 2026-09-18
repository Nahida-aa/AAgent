//! Panel 定义 + PanelKind 枚举 + PanelEntry。

use gpui::App;
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
    /// Zed default.json 里的 settings key（用于 SettingsStore 读取）。
    /// Zed 里 dock 位置的 key: project_panel, git_panel, outline_panel,
    /// collab_panel, agent, terminal, debug（不完全一致，有的带 _panel 有的不带）。
    pub fn settings_key(self) -> &'static str {
        match self {
            PanelKind::Project => "project_panel",
            PanelKind::Git => "git_panel",
            PanelKind::Collab => "collab_panel",
            PanelKind::Outline => "outline_panel",
            PanelKind::Terminal => "terminal",
            PanelKind::Debug => "debug",
            PanelKind::Agent => "agent",
        }
    }

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

    /// 默认 dock 位置。
    ///
    /// 有 App 上下文时从 SettingsStore 读取（对应 default.json 里的
    /// `project_panel.dock` / `git_panel.dock` / `agent.dock` 等 key）；
    /// 没有 App 上下文时 fallback 硬编码默认值（对齐 Zed classic 布局）。
    ///
    /// Zed classic 布局 (default.json):
    /// - Project / Git / Collab / Outline → Right
    /// - Agent → Left
    /// - Terminal / Debug → Bottom
    pub fn default_position(self, cx: Option<&App>) -> DockPosition {
        // 尝试从 SettingsStore 读取 default.json + 用户覆盖
        if let Some(cx) = cx {
            if let Some(store) = cx.try_global::<settings::SettingsStore>() {
                let name = self.settings_key();
                let path: [&str; 2] = [name, "dock"];
                if let Ok(dock_str) = store.try_get_path::<String>(&path) {
                    return parse_dock_position(&dock_str);
                }
            }
        }
        // Fallback: 硬编码默认值
        self.hardcoded_default()
    }

    fn hardcoded_default(self) -> DockPosition {
        match self {
            PanelKind::Project | PanelKind::Git | PanelKind::Collab | PanelKind::Outline => {
                DockPosition::Right
            }
            PanelKind::Agent => DockPosition::Left,
            PanelKind::Terminal | PanelKind::Debug => DockPosition::Bottom,
        }
    }

    /// 这个面板能不能放到指定 Dock 位置（对齐 zed `Panel::position_is_valid`）。
    ///
    /// Zed 各面板的策略（只列差异的）：
    /// - Project / Git / Collab / Outline → Left | Right（侧边栏，不能底部）
    /// - Agent → != Bottom（左右，不能底部）
    /// - Terminal / Debugger → 全部三个
    pub fn position_is_valid(self, position: DockPosition) -> bool {
        match self {
            PanelKind::Project | PanelKind::Git | PanelKind::Collab | PanelKind::Outline => {
                matches!(position, DockPosition::Left | DockPosition::Right)
            }
            PanelKind::Agent => position != DockPosition::Bottom,
            PanelKind::Terminal | PanelKind::Debug => true,
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

    pub fn default_position(&self, cx: Option<&App>) -> DockPosition {
        self.kind.default_position(cx)
    }
}

/// 从 settings JSON 的 dock 字符串解析 DockPosition。
fn parse_dock_position(s: &str) -> DockPosition {
    match s.to_ascii_lowercase().as_str() {
        "left" => DockPosition::Left,
        "right" => DockPosition::Right,
        "bottom" => DockPosition::Bottom,
        _ => DockPosition::Right, // 默认右侧
    }
}
