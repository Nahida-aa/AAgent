//! 项目级 terminal settings — 可被 `.aa/settings.json` 覆盖。
//!
//! 单独拆模块是因为 Zed 用 `#[serde(flatten)]` 把这个 struct
//! 内嵌进顶层 `TerminalSettingsContent`，逻辑上是两层 settings。
//!
//! 本模块还定义了 `Shell` enum（纯 serde 数据类型 + runtime 方法），
//! 它依赖 `util::shell::ShellKind` 做检测。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

// ---------- Shell ----------

/// Shell configuration to open the terminal with.
///
/// 放在 settings-content 层（纯 serde 数据），runtime 方法 `shell_kind()` 等
/// 会调 `util::shell::ShellKind::new()` 做检测。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    /// 用系统默认（Linux/macOS 读 $SHELL，Windows cmd.exe）。
    #[default]
    System,
    /// 用指定 program，不带参数。
    Program(String),
    /// 用指定 program + arguments。
    WithArguments {
        program: String,
        args: Vec<String>,
        title_override: Option<String>,
    },
}

impl Shell {
    /// 返回要执行的 program（System 时走系统检测）。
    pub fn program(&self) -> String {
        match self {
            Shell::Program(program) => program.clone(),
            Shell::WithArguments { program, .. } => program.clone(),
            Shell::System => util::shell::get_system_shell(),
        }
    }

    /// 返回 `(program, args)` — System 时 args 是空 slice。
    pub fn program_and_args(&self) -> (String, &[String]) {
        match self {
            Shell::Program(program) => (program.clone(), &[]),
            Shell::WithArguments { program, args, .. } => (program.clone(), args),
            Shell::System => (util::shell::get_system_shell(), &[]),
        }
    }

    /// 识别 shell 类型（Posix / PowerShell / Cmd / Nushell ...）。
    pub fn shell_kind(&self, is_windows: bool) -> util::shell::ShellKind {
        match self {
            Shell::Program(program) => util::shell::ShellKind::new(program, is_windows),
            Shell::WithArguments { program, .. } => {
                util::shell::ShellKind::new(program, is_windows)
            }
            Shell::System => util::shell::ShellKind::system(),
        }
    }
}

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

/// 项目级 terminal settings — 被 `serde(flatten)` 进 TerminalSettingsContent。
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
