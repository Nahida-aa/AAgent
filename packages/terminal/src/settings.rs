//! Terminal 相关 settings 类型。
//!
//! 对齐 Zed `crates/settings_content/src/terminal.rs`。
//!
//! 类型分为两组：
//! - 项目级（ProjectTerminalSettingsContent）— 可在 `.aa/settings.json` 里覆盖
//! - 全局级（TerminalSettingsContent）— 用户 settings.json 里的 terminal 块
//!
//! `Shell` 复用 `util::shell::Shell`（同一个 enum，不用重复定义）。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

use settings_content::{FontFamilyName, FontSize, FontWeightContent};
use util::shell::Shell;

// ---------- WorkingDirectory ----------

/// Terminal 启动时的工作目录策略。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum WorkingDirectory {
    /// 当前文件目录 → fallback 项目目录 → fallback workspace 第一个项目
    CurrentFileDirectory,
    /// 当前文件所属项目目录
    CurrentProjectDirectory,
    /// workspace 第一个项目目录 → fallback home
    #[default]
    FirstProjectDirectory,
    /// 总是 home 目录
    AlwaysHome,
    /// 总是指定目录（会被 shell expand）
    Always { directory: String },
}

// ---------- ActivateScript ----------

/// Python venv activate 脚本类型。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum ActivateScript {
    #[default]
    Default,
    Csh,
    Fish,
    Nushell,
    PowerShell,
    Pyenv,
}

// ---------- CondaManager ----------

/// Conda 环境激活时优先用哪个管理器。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum CondaManager {
    #[default]
    Auto,
    Conda,
    Mamba,
    Micromamba,
}

// ---------- VenvSettings ----------

/// Python 虚拟环境自动激活设置。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum VenvSettings {
    #[default]
    Off,
    On {
        #[serde(default)]
        activate_script: Option<ActivateScript>,
        #[serde(default)]
        venv_name: Option<String>,
        #[serde(default)]
        directories: Option<Vec<PathBuf>>,
        #[serde(default)]
        conda_manager: Option<CondaManager>,
    },
}

/// VenvSettings 展开后的值（运行时用，避免 Option 链）。
#[derive(Clone, Debug, Default, PartialEq, Eq, MergeFrom)]
pub struct VenvSettingsResolved {
    pub activate_script: ActivateScript,
    pub venv_name: String,
    pub directories: Vec<PathBuf>,
    pub conda_manager: CondaManager,
}

impl VenvSettings {
    pub fn resolve(&self) -> Option<VenvSettingsResolved> {
        match self {
            VenvSettings::Off => None,
            VenvSettings::On {
                activate_script,
                venv_name,
                directories,
                conda_manager,
            } => Some(VenvSettingsResolved {
                activate_script: activate_script.unwrap_or_default(),
                venv_name: venv_name.clone().unwrap_or_default(),
                directories: directories.clone().unwrap_or_default(),
                conda_manager: conda_manager.unwrap_or_default(),
            }),
        }
    }
}

// ---------- PathHyperlinkRegex ----------

/// 用于识别路径超链接的 regex（单行或多行数组）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, MergeFrom)]
#[serde(untagged)]
pub enum PathHyperlinkRegex {
    SingleLine(String),
    MultiLine(Vec<String>),
}

// ---------- ProjectTerminalSettingsContent ----------

/// 项目级 terminal settings（可被 `.aa/settings.json` 覆盖）。
#[with_fallible_options]
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, MergeFrom)]
pub struct ProjectTerminalSettingsContent {
    /// Shell — 复用 `util::shell::Shell`。
    pub shell: Option<Shell>,
    /// 工作目录策略。
    pub working_directory: Option<WorkingDirectory>,
    /// 环境变量（会加进 terminal 进程环境）。
    pub env: Option<HashMap<String, String>>,
    /// Python venv 自动激活。
    pub detect_venv: Option<VenvSettings>,
    /// 路径超链接 regex 列表。
    pub path_hyperlink_regexes: Option<Vec<PathHyperlinkRegex>>,
    /// 超链接发现超时（ms）。
    pub path_hyperlink_timeout_ms: Option<u64>,
}

// ---------- TerminalLineHeight ----------

/// Terminal 行高。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum TerminalLineHeight {
    /// 舒适行高 1.618（黄金比例）。
    #[default]
    Comfortable,
    /// 标准行高 1.3（TUIs / box characters 更准）。
    Standard,
    /// 自定义。
    Custom(f32),
}

impl TerminalLineHeight {
    pub fn value(&self) -> f32 {
        match self {
            TerminalLineHeight::Comfortable => 1.618,
            TerminalLineHeight::Standard => 1.3,
            TerminalLineHeight::Custom(v) => v.max(1.0),
        }
    }
}

// ---------- CursorShapeContent ----------

/// Terminal 光标形状。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum CursorShapeContent {
    /// 实心方块 █
    #[default]
    Block,
    /// 下划线 _
    Underline,
    /// 竖线 ⎸
    Bar,
    /// 空心方块 ▯
    Hollow,
}

// ---------- TerminalBlink ----------

/// 光标闪烁行为。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBlink {
    /// 终端控制 — 关闭为默认，但允许应用开启
    #[default]
    TerminalControlled,
    Off,
    On,
}

// ---------- AlternateScroll ----------

/// Alternate Scroll mode（?1007）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum AlternateScroll {
    #[default]
    On,
    Off,
}

// ---------- ShowScrollbar ----------

/// Terminal scrollbar 显示策略。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum ShowScrollbar {
    /// 继承 editor 设置
    #[default]
    Auto,
    System,
    Always,
    Never,
}

// ---------- ScrollbarSettingsContent ----------

#[with_fallible_options]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, MergeFrom)]
pub struct ScrollbarSettingsContent {
    pub show: Option<ShowScrollbar>,
}

// ---------- TerminalToolbarContent ----------

#[with_fallible_options]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, MergeFrom)]
pub struct TerminalToolbarContent {
    /// 是否在 terminal pane 内显示 breadcrumbs（需要 shell emit OSC 2 title）。
    ///
    /// 默认：true
    pub breadcrumbs: Option<bool>,
}

// ---------- TerminalBell ----------

/// BEL (`\a`) 行为。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBell {
    #[default]
    System,
    Off,
}

// ---------- TerminalDockPosition ----------

/// Terminal dock 位置。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum TerminalDockPosition {
    Left,
    #[default]
    Bottom,
    Right,
}

// ---------- TerminalSettingsContent ----------

/// Terminal settings 顶层 — 被 `#[serde(flatten)]` 内嵌 project 级设置。
///
/// 注意：不能加 `#[with_fallible_options]` —— serde flatten 与该 macro
/// 自动追加的字段级 `#[serde(default)]` 冲突。
/// 改为 struct 级 `#[serde(default)]`，所有字段本来就是 `Option<T>`。
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, MergeFrom)]
#[serde(default)]
pub struct TerminalSettingsContent {
    /// 项目级设置（shell、working_directory、env、venv、path_hyperlink）。
    #[serde(flatten)]
    pub project: ProjectTerminalSettingsContent,

    // ---- 字体 ----
    /// 未设时跟随 buffer。
    pub font_size: Option<FontSize>,
    pub font_family: Option<FontFamilyName>,
    pub font_weight: Option<FontWeightContent>,
    /// 未设时跟随 buffer。
    pub font_fallbacks: Option<Vec<FontFamilyName>>,
    /// 默认：Comfortable
    pub line_height: Option<TerminalLineHeight>,

    // ---- 光标 ----
    /// 默认：Block
    pub cursor_shape: Option<CursorShapeContent>,
    /// 默认：TerminalControlled
    pub blinking: Option<TerminalBlink>,
    /// 默认：On
    pub alternate_scroll: Option<AlternateScroll>,

    // ---- 交互 ----
    /// 默认：false
    pub option_as_meta: Option<bool>,
    /// 默认：false
    pub copy_on_select: Option<bool>,
    /// 默认：true
    pub keep_selection_on_copy: Option<bool>,
    /// 默认：true（Ctrl/Cmd+点击打开超链接，即使 terminal app 开启 mouse reporting）
    pub open_links_in_mouse_mode: Option<bool>,

    // ---- Dock 相关 ----
    /// 默认：true
    pub button: Option<bool>,
    pub dock: Option<TerminalDockPosition>,
    /// 默认：false
    pub starts_open: Option<bool>,
    /// 默认：true
    pub flexible: Option<bool>,
    /// 默认：640
    pub default_width: Option<f32>,
    /// 默认：320
    pub default_height: Option<f32>,

    // ---- Scrolling ----
    /// 默认：10_000（最大 100_000）
    pub max_scroll_history_lines: Option<usize>,
    /// 默认：1.0
    pub scroll_multiplier: Option<f32>,

    // ---- UI 细项 ----
    pub toolbar: Option<TerminalToolbarContent>,
    pub scrollbar: Option<ScrollbarSettingsContent>,
    /// 默认：false（terminal 面板图标上显示 terminal 数量 badge）
    pub show_count_badge: Option<bool>,
    /// 默认：System
    pub bell: Option<TerminalBell>,

    // ---- 可访问性 ----
    /// APCA 最小对比度 0-106，默认 45。
    #[serde(default)]
    pub minimum_contrast: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_terminal_settings_default() {
        let s: TerminalSettingsContent = serde_json::from_value(json!({})).unwrap();
        assert!(s.font_size.is_none());
        assert!(s.project.shell.is_none());
    }

    #[test]
    fn test_project_shell() {
        // Shell 用 #[serde(rename_all = "snake_case")] + internally-tagged enum
        // System     → "shell": "system"
        // Program    → "shell": { "program": "..." }  (unit variant + data field)
        // WithArguments → "shell": { "with_arguments": { "program": "...", "args": [...] } }
        let s: TerminalSettingsContent = serde_json::from_value(json!({
            "shell": { "with_arguments": { "program": "/bin/zsh", "args": ["-l"] } }
        }))
        .unwrap();
        match &s.project.shell {
            Some(util::shell::Shell::WithArguments { program, args, .. }) => {
                assert_eq!(program, "/bin/zsh");
                assert_eq!(args, &["-l"]);
            }
            other => panic!("expected WithArguments, got {:?}", other),
        }
    }

    #[test]
    fn test_terminal_line_height() {
        assert_eq!(TerminalLineHeight::Comfortable.value(), 1.618);
        assert_eq!(TerminalLineHeight::Standard.value(), 1.3);
        assert_eq!(TerminalLineHeight::Custom(2.0).value(), 2.0);
        assert_eq!(TerminalLineHeight::Custom(0.5).value(), 1.0); // 最小 clamp
    }

    #[test]
    fn test_cursor_shape_deserialize() {
        let s: TerminalSettingsContent = serde_json::from_value(json!({
            "cursor_shape": "bar"
        }))
        .unwrap();
        assert_eq!(s.cursor_shape, Some(CursorShapeContent::Bar));
    }

    #[test]
    fn test_venv_on() {
        let s: TerminalSettingsContent = serde_json::from_value(json!({
            "detect_venv": {
                "on": {
                    "conda_manager": "mamba",
                    "venv_name": ".venv"
                }
            }
        }))
        .unwrap();
        let resolved = s.project.detect_venv.unwrap().resolve().unwrap();
        assert_eq!(resolved.conda_manager, CondaManager::Mamba);
        assert_eq!(resolved.venv_name, ".venv");
    }

    #[test]
    fn test_working_directory() {
        let s: TerminalSettingsContent = serde_json::from_value(json!({
            "working_directory": { "always": { "directory": "~/work" } }
        }))
        .unwrap();
        assert!(matches!(
            s.project.working_directory,
            Some(WorkingDirectory::Always { .. })
        ));
    }

    #[test]
    fn test_dock_position() {
        let s: TerminalSettingsContent = serde_json::from_value(json!({
            "dock": "right"
        }))
        .unwrap();
        assert_eq!(s.dock, Some(TerminalDockPosition::Right));
    }
}
