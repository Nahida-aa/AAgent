//! Shell 检测 + quoting + 命令构造。
//!
//! 对齐 Zed `crates/util/src/shell.rs`，但去掉了 `schemars::JsonSchema`、
//! `gpui_util`（Windows shell 检测）、`brush-parser` 相关引用。
//!
//! 核心类型：
//! - [`Shell`] — 用户 settings 里配置的 shell（System / Program / WithArguments）
//! - [`ShellKind`] — 识别出的 shell 类型（Posix/Fish/PowerShell/Pwsh/Cmd/Nushell/...）

use std::borrow::Cow;
use std::fmt;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use settings_macros::MergeFrom;

// ---------- Shell ----------

/// Shell configuration — 用户 settings 里用来指定用哪个 shell 打开 terminal。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    /// 用系统默认（读取 $SHELL 或 Windows 注册表）。
    #[default]
    System,
    /// 用指定 program，不带参数。
    Program(String),
    /// 用指定 program + arguments。
    WithArguments {
        /// 程序路径 / 名字。
        program: String,
        /// 传给程序的参数。
        args: Vec<String>,
        /// 可选的 terminal tab title override。
        title_override: Option<String>,
    },
}

impl Shell {
    /// 返回要执行的 program（System 时走系统检测）。
    pub fn program(&self) -> String {
        match self {
            Shell::Program(program) => program.clone(),
            Shell::WithArguments { program, .. } => program.clone(),
            Shell::System => get_system_shell(),
        }
    }

    /// 返回 `(program, args)` — System 时 args 是空 slice。
    pub fn program_and_args(&self) -> (String, &[String]) {
        match self {
            Shell::Program(program) => (program.clone(), &[]),
            Shell::WithArguments { program, args, .. } => (program.clone(), args),
            Shell::System => (get_system_shell(), &[]),
        }
    }

    /// 识别 shell 类型（Posix / PowerShell / Cmd / Nushell / ...）。
    pub fn shell_kind(&self, is_windows: bool) -> ShellKind {
        match self {
            Shell::Program(program) => ShellKind::new(program, is_windows),
            Shell::WithArguments { program, .. } => ShellKind::new(program, is_windows),
            Shell::System => ShellKind::system(),
        }
    }
}

// ---------- ShellKind ----------

/// Shell 类型枚举，用于决定 quoting 规则、命令 separator、变量语法等。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellKind {
    #[default]
    Posix,
    Csh,
    Tcsh,
    Rc,
    Fish,
    /// 旧版 Windows PowerShell 5.x
    PowerShell,
    /// PowerShell 7+
    Pwsh,
    Nushell,
    Cmd,
    Xonsh,
    Elvish,
}

impl ShellKind {
    /// 基于系统默认 shell 检测类型。
    pub fn system() -> Self {
        Self::new(&get_system_shell(), cfg!(windows))
    }

    /// 根据 program 名 / 路径推断 ShellKind。
    ///
    /// 匹配顺序：先查常见 shell 名（powershell/pwsh/cmd/nu/fish/csh/tcsh/rc/xonsh/elvish/sh/bash/zsh），
    /// 未识别时：Windows 上默认 PowerShell，其他默认 Posix。
    pub fn new(program: impl AsRef<Path>, is_windows: bool) -> Self {
        let program = program.as_ref();
        let program = program
            .file_stem()
            .unwrap_or_else(|| program.as_os_str())
            .to_string_lossy();

        match &*program {
            "powershell" => ShellKind::PowerShell,
            "pwsh" => ShellKind::Pwsh,
            "cmd" => ShellKind::Cmd,
            "nu" => ShellKind::Nushell,
            "fish" => ShellKind::Fish,
            "csh" => ShellKind::Csh,
            "tcsh" => ShellKind::Tcsh,
            "rc" => ShellKind::Rc,
            "xonsh" => ShellKind::Xonsh,
            "elvish" => ShellKind::Elvish,
            "sh" | "bash" | "zsh" => ShellKind::Posix,
            _ if is_windows => ShellKind::PowerShell,
            _ => ShellKind::Posix,
        }
    }

    /// 生成执行命令的参数列表（`-c "..."` / `/S /C "..."` / `-C "..."` 等）。
    pub fn args_for_shell(&self, interactive: bool, combined_command: String) -> Vec<String> {
        match self {
            ShellKind::PowerShell | ShellKind::Pwsh => vec!["-C".to_owned(), combined_command],
            ShellKind::Cmd => vec![
                "/S".to_owned(),
                "/C".to_owned(),
                format!("\"{combined_command}\""),
            ],
            ShellKind::Posix
            | ShellKind::Nushell
            | ShellKind::Fish
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Xonsh
            | ShellKind::Elvish => interactive
                .then(|| "-i".to_owned())
                .into_iter()
                .chain(["-c".to_owned(), combined_command])
                .collect(),
        }
    }

    /// 命令前缀字符（PowerShell/Pwsh=`&`, Nushell=`^`, 其他=None）。
    pub const fn command_prefix(&self) -> Option<char> {
        match self {
            ShellKind::PowerShell | ShellKind::Pwsh => Some('&'),
            ShellKind::Nushell => Some('^'),
            ShellKind::Posix
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Fish
            | ShellKind::Cmd
            | ShellKind::Xonsh
            | ShellKind::Elvish => None,
        }
    }

    /// 顺序命令 separator（Cmd=`&`, 其他=`;`）。
    pub const fn sequential_commands_separator(&self) -> char {
        match self {
            ShellKind::Cmd => '&',
            ShellKind::Posix
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Fish
            | ShellKind::PowerShell
            | ShellKind::Pwsh
            | ShellKind::Nushell
            | ShellKind::Xonsh
            | ShellKind::Elvish => ';',
        }
    }

    /// `&&` separator — Nushell / PowerShell / Elvish 不支持（用 `;`）。
    pub const fn sequential_and_commands_separator(&self) -> &'static str {
        match self {
            ShellKind::Cmd
            | ShellKind::Posix
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Fish
            | ShellKind::Pwsh
            | ShellKind::Xonsh => "&&",
            ShellKind::PowerShell | ShellKind::Nushell | ShellKind::Elvish => ";",
        }
    }

    /// 激活脚本的关键字（PowerShell/Pwsh=`.`, Fish=source, Cmd="" 等）。
    pub const fn activate_keyword(&self) -> &'static str {
        match self {
            ShellKind::Cmd => "",
            ShellKind::Nushell => "overlay use",
            ShellKind::PowerShell | ShellKind::Pwsh => ".",
            ShellKind::Fish
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Posix
            | ShellKind::Rc
            | ShellKind::Xonsh
            | ShellKind::Elvish => "source",
        }
    }

    /// 清屏命令。
    pub const fn clear_screen_command(&self) -> &'static str {
        match self {
            ShellKind::Cmd => "cls",
            ShellKind::Posix
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Fish
            | ShellKind::PowerShell
            | ShellKind::Pwsh
            | ShellKind::Nushell
            | ShellKind::Xonsh
            | ShellKind::Elvish => "clear",
        }
    }

    /// 将 `$VAR` / `${VAR}` 形式的 shell 变量转成当前 shell 的语法。
    /// - PowerShell/Pwsh → `$env:VAR`
    /// - Cmd → `%VAR%`
    /// - Nushell → `$env.VAR`
    /// - 其他 → 原样（POSIX 本来就是 `${VAR}`）
    pub fn to_shell_variable(self, input: &str) -> String {
        match self {
            Self::PowerShell | Self::Pwsh => Self::to_powershell_variable(input),
            Self::Cmd => Self::to_cmd_variable(input),
            Self::Posix => input.to_owned(),
            Self::Fish => input.to_owned(),
            Self::Csh => input.to_owned(),
            Self::Tcsh => input.to_owned(),
            Self::Rc => input.to_owned(),
            Self::Nushell => Self::to_nushell_variable(input),
            Self::Xonsh => input.to_owned(),
            Self::Elvish => input.to_owned(),
        }
    }

    fn to_cmd_variable(input: &str) -> String {
        if let Some(var_str) = input.strip_prefix("${") {
            match var_str.strip_suffix('}') {
                Some(var_name) if !var_name.is_empty() && !var_name.contains(':') => {
                    format!("%{var_name}%")
                }
                _ => input.into(),
            }
        } else if let Some(var_str) = input.strip_prefix('$') {
            format!("%{}%", var_str)
        } else {
            input.into()
        }
    }

    fn to_powershell_variable(input: &str) -> String {
        if let Some(var_str) = input.strip_prefix("${") {
            match var_str.strip_suffix('}') {
                Some(var_name) if !var_name.is_empty() && !var_name.contains(':') => {
                    format!("$env:{var_name}")
                }
                _ => input.into(),
            }
        } else if let Some(var_str) = input.strip_prefix('$') {
            format!("$env:{}", var_str)
        } else {
            input.into()
        }
    }

    fn to_nushell_variable(input: &str) -> String {
        let mut result = String::new();
        let mut source = input;
        let mut is_start = true;

        loop {
            match source.chars().next() {
                None => return result,
                Some('$') => {
                    source = Self::parse_nushell_var(&source[1..], &mut result, is_start);
                    is_start = false;
                }
                Some(_) => {
                    is_start = false;
                    let chunk_end = source.find('$').unwrap_or(source.len());
                    let (chunk, rest) = source.split_at(chunk_end);
                    result.push_str(chunk);
                    source = rest;
                }
            }
        }
    }

    fn parse_nushell_var<'a>(source: &'a str, text: &mut String, is_start: bool) -> &'a str {
        if source.starts_with("env.") {
            text.push('$');
            return source;
        }

        match source.chars().next() {
            Some('{') => {
                let source = &source[1..];
                if let Some(end) = source.find('}') {
                    let var_name = &source[..end];
                    if !var_name.is_empty() {
                        if !is_start {
                            text.push_str("(");
                        }
                        text.push_str("$env.");
                        text.push_str(var_name);
                        if !is_start {
                            text.push_str(")");
                        }
                        &source[end + 1..]
                    } else {
                        text.push_str("${}");
                        &source[end + 1..]
                    }
                } else {
                    text.push_str("${");
                    source
                }
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let end = source
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(source.len());
                let var_name = &source[..end];
                if !is_start {
                    text.push_str("(");
                }
                text.push_str("$env.");
                text.push_str(var_name);
                if !is_start {
                    text.push_str(")");
                }
                &source[end..]
            }
            _ => {
                text.push('$');
                source
            }
        }
    }

    /// Quoting — POSIX 系走 shlex，Windows PowerShell/Cmd 各有规则。
    pub fn try_quote<'a>(&self, arg: &'a str) -> Option<Cow<'a, str>> {
        match self {
            ShellKind::PowerShell => Some(Self::quote_powershell(arg)),
            ShellKind::Pwsh => Some(Self::quote_pwsh(arg)),
            ShellKind::Cmd => Some(Self::quote_cmd(arg)),
            ShellKind::Posix
            | ShellKind::Csh
            | ShellKind::Tcsh
            | ShellKind::Rc
            | ShellKind::Fish
            | ShellKind::Nushell
            | ShellKind::Xonsh
            | ShellKind::Elvish => shlex::try_quote(arg).ok(),
        }
    }

    /// Windows CMD/PowerShell 通用的 CRT 级 quoting（处理反斜杠 + 引号规则）。
    fn quote_windows(arg: &str, enclose: bool) -> Cow<'_, str> {
        if arg.is_empty() {
            return Cow::Borrowed("\"\"");
        }

        let needs_quoting = arg.chars().any(|c| c == ' ' || c == '\t' || c == '"');
        if !needs_quoting {
            return Cow::Borrowed(arg);
        }

        let mut result = String::with_capacity(arg.len() + 2);
        if enclose {
            result.push('"');
        }

        let chars: Vec<char> = arg.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '\\' {
                let mut num_backslashes = 0;
                while i < chars.len() && chars[i] == '\\' {
                    num_backslashes += 1;
                    i += 1;
                }

                if i < chars.len() && chars[i] == '"' {
                    for _ in 0..(num_backslashes * 2 + 1) {
                        result.push('\\');
                    }
                    result.push('"');
                    i += 1;
                } else if i >= chars.len() {
                    for _ in 0..(num_backslashes * 2) {
                        result.push('\\');
                    }
                } else {
                    for _ in 0..num_backslashes {
                        result.push('\\');
                    }
                }
            } else if chars[i] == '"' {
                result.push('\\');
                result.push('"');
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        if enclose {
            result.push('"');
        }
        Cow::Owned(result)
    }

    fn needs_quoting_powershell(s: &str) -> bool {
        s.is_empty()
            || s.chars().any(|c| {
                c.is_whitespace()
                    || matches!(
                        c,
                        '"' | '`'
                            | '$'
                            | '&'
                            | '|'
                            | '<'
                            | '>'
                            | ';'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | ','
                            | '\''
                            | '@'
                    )
            })
    }

    fn need_quotes_powershell(arg: &str) -> bool {
        let mut quote_count = 0;
        for c in arg.chars() {
            if c == '"' {
                quote_count += 1;
            } else if c.is_whitespace() && (quote_count % 2 == 0) {
                return true;
            }
        }
        false
    }

    fn escape_powershell_quotes(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 4);
        result.push('\'');
        for c in s.chars() {
            if c == '\'' {
                result.push('\'');
            }
            result.push(c);
        }
        result.push('\'');
        result
    }

    pub fn quote_powershell(arg: &str) -> Cow<'_, str> {
        let ps_will_quote = Self::need_quotes_powershell(arg);
        let crt_quoted = Self::quote_windows(arg, !ps_will_quote);

        if !Self::needs_quoting_powershell(arg) {
            return crt_quoted;
        }
        Cow::Owned(Self::escape_powershell_quotes(&crt_quoted))
    }

    pub fn quote_pwsh(arg: &str) -> Cow<'_, str> {
        if arg.is_empty() {
            return Cow::Borrowed("''");
        }
        if !Self::needs_quoting_powershell(arg) {
            return Cow::Borrowed(arg);
        }
        Cow::Owned(Self::escape_powershell_quotes(arg))
    }

    pub fn quote_cmd(arg: &str) -> Cow<'_, str> {
        let crt_quoted = Self::quote_windows(arg, true);
        let needs_cmd_escaping = crt_quoted.contains(['"', '%', '^', '<', '>', '&', '|', '(', ')']);

        if !needs_cmd_escaping {
            return crt_quoted;
        }

        let mut result = String::with_capacity(crt_quoted.len() * 2);
        for c in crt_quoted.chars() {
            match c {
                '^' | '"' | '<' | '>' | '&' | '|' | '(' | ')' => {
                    result.push('^');
                    result.push(c);
                }
                '%' => result.push_str("%%cd:~,%"),
                _ => result.push(c),
            }
        }
        Cow::Owned(result)
    }

    /// shlex split — 只有 POSIX/Nushell/Fish 类 shell 用。
    pub fn split(&self, input: &str) -> Option<Vec<String>> {
        shlex::split(input)
    }

    /// 命令前缀 + quoting aware — PowerShell `&` / Nushell `^` 不被当成 shell 语法吃掉。
    pub fn prepend_command_prefix<'a>(&self, command: &'a str) -> Cow<'a, str> {
        match self.command_prefix() {
            Some(prefix) if !command.starts_with(prefix) => {
                Cow::Owned(format!("{prefix}{command}"))
            }
            _ => Cow::Borrowed(command),
        }
    }

    /// 判断 shell 命令 chaining 语法是否能被 brush-parser 解析（未来 terminal 安全审计用）。
    pub fn supports_posix_chaining(&self) -> bool {
        matches!(
            self,
            ShellKind::Posix
                | ShellKind::Fish
                | ShellKind::PowerShell
                | ShellKind::Pwsh
                | ShellKind::Cmd
                | ShellKind::Xonsh
                | ShellKind::Csh
                | ShellKind::Tcsh
                | ShellKind::Nushell
                | ShellKind::Elvish
                | ShellKind::Rc
        )
    }
}

impl fmt::Display for ShellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellKind::Posix => write!(f, "sh"),
            ShellKind::Csh => write!(f, "csh"),
            ShellKind::Tcsh => write!(f, "tcsh"),
            ShellKind::Fish => write!(f, "fish"),
            ShellKind::PowerShell => write!(f, "powershell"),
            ShellKind::Pwsh => write!(f, "pwsh"),
            ShellKind::Nushell => write!(f, "nu"),
            ShellKind::Cmd => write!(f, "cmd"),
            ShellKind::Rc => write!(f, "rc"),
            ShellKind::Xonsh => write!(f, "xonsh"),
            ShellKind::Elvish => write!(f, "elvish"),
        }
    }
}

// ---------- 系统 shell 检测 ----------

/// 检测系统 shell：Linux/macOS 读 `$SHELL`，回退 `/bin/sh`；Windows 默认 cmd.exe。
pub fn get_system_shell() -> String {
    if cfg!(windows) {
        get_windows_system_shell()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// 返回平台默认 shell（不读 env）。
pub fn get_default_system_shell() -> String {
    if cfg!(windows) {
        get_windows_system_shell()
    } else {
        "/bin/sh".to_string()
    }
}

/// 默认 shell，Windows 上优先 Git Bash。
pub fn get_default_system_shell_preferring_bash() -> String {
    #[cfg(windows)]
    {
        get_windows_bash().unwrap_or_else(get_windows_system_shell)
    }
    #[cfg(not(windows))]
    {
        "/bin/sh".to_string()
    }
}

/// Windows 上找 Git 自带的 bash.exe（优先）。
#[cfg(windows)]
pub fn get_windows_bash() -> Option<String> {
    fn find_bash_in_installation(install_root: &Path) -> Option<PathBuf> {
        if !install_root.join("git-bash.exe").is_file() {
            return None;
        }
        let bash = install_root.join("bin").join("bash.exe");
        bash.is_file().then_some(bash)
    }

    static BASH: LazyLock<Option<String>> = LazyLock::new(|| {
        let bash = std::env::var_os("GIT_INSTALL_ROOT")
            .map(PathBuf::from)
            .and_then(|root| find_bash_in_installation(&root))
            .or_else(|| {
                let git = which::which("git").ok()?;
                let binary_dir = git.parent()?;
                let parent = binary_dir.parent()?;
                find_bash_in_installation(parent)
                    .or_else(|| find_bash_in_installation(parent.parent()?))
            })
            .map(|p| p.to_string_lossy().into_owned());
        if let Some(ref path) = bash {
            log::info!("Found bash at {}", path);
        }
        bash
    });

    BASH.clone()
}

/// Windows 系统 shell（默认 cmd.exe，以后可以从注册表读）。
pub fn get_windows_system_shell() -> String {
    #[cfg(windows)]
    return "cmd.exe".to_string();

    #[cfg(not(windows))]
    return "cmd.exe".to_string();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_kind_detection_linux() {
        assert_eq!(ShellKind::new("/bin/bash", false), ShellKind::Posix);
        assert_eq!(ShellKind::new("/bin/zsh", false), ShellKind::Posix);
        assert_eq!(ShellKind::new("/usr/bin/fish", false), ShellKind::Fish);
        assert_eq!(ShellKind::new("nu", false), ShellKind::Nushell);
        assert_eq!(ShellKind::new("unknown", false), ShellKind::Posix);
        assert_eq!(ShellKind::new("unknown", true), ShellKind::PowerShell);
    }

    #[test]
    fn test_shell_kind_detection_windows() {
        // 注意：在 Unix 平台上 file_stem 会把 \\ 当作普通字符，
        // 所以只用 program 名测试跨平台路径
        assert_eq!(ShellKind::new("cmd.exe", true), ShellKind::Cmd);
        assert_eq!(
            ShellKind::new("powershell.exe", true),
            ShellKind::PowerShell
        );
        assert_eq!(ShellKind::new("pwsh.exe", true), ShellKind::Pwsh);
    }

    #[test]
    fn test_to_shell_variable() {
        assert_eq!(
            ShellKind::PowerShell.to_shell_variable("${FOO}"),
            "$env:FOO"
        );
        assert_eq!(ShellKind::Pwsh.to_shell_variable("${FOO}"), "$env:FOO");
        assert_eq!(ShellKind::Cmd.to_shell_variable("${FOO}"), "%FOO%");
        assert_eq!(ShellKind::Nushell.to_shell_variable("${FOO}"), "$env.FOO");
        assert_eq!(ShellKind::Posix.to_shell_variable("${FOO}"), "${FOO}");
        assert_eq!(ShellKind::PowerShell.to_shell_variable("$FOO"), "$env:FOO");
        // malformed — passthrough
        assert_eq!(
            ShellKind::PowerShell.to_shell_variable("${FOO:-bar}"),
            "${FOO:-bar}"
        );
    }

    #[test]
    fn test_args_for_shell() {
        // PowerShell/Pwsh
        assert_eq!(
            ShellKind::PowerShell.args_for_shell(false, "echo hi".to_string()),
            vec!["-C", "echo hi"]
        );
        // Cmd
        assert_eq!(
            ShellKind::Cmd.args_for_shell(false, "echo hi".to_string()),
            vec!["/S", "/C", "\"echo hi\""]
        );
        // Posix non-interactive
        assert_eq!(
            ShellKind::Posix.args_for_shell(false, "echo hi".to_string()),
            vec!["-c", "echo hi"]
        );
        // Posix interactive
        assert_eq!(
            ShellKind::Posix.args_for_shell(true, "echo hi".to_string()),
            vec!["-i", "-c", "echo hi"]
        );
    }

    #[test]
    fn test_command_prefix() {
        assert_eq!(ShellKind::PowerShell.command_prefix(), Some('&'));
        assert_eq!(ShellKind::Pwsh.command_prefix(), Some('&'));
        assert_eq!(ShellKind::Nushell.command_prefix(), Some('^'));
        assert_eq!(ShellKind::Posix.command_prefix(), None);
        assert_eq!(ShellKind::Cmd.command_prefix(), None);
    }

    #[test]
    fn test_sequential_separator() {
        assert_eq!(ShellKind::Posix.sequential_commands_separator(), ';');
        assert_eq!(ShellKind::Cmd.sequential_commands_separator(), '&');
        assert_eq!(ShellKind::PowerShell.sequential_commands_separator(), ';');
    }

    #[test]
    fn test_posix_quoting() {
        // "hello world" — 单引号 quoting，shlex 拆出一个 token
        let quoted = ShellKind::Posix.try_quote("hello world").unwrap();
        assert_eq!(shlex::split(&quoted), Some(vec!["hello world".to_string()]));
        let quoted = ShellKind::Posix.try_quote("O'Brien").unwrap();
        assert_eq!(shlex::split(&quoted), Some(vec!["O'Brien".to_string()]));
    }

    #[test]
    fn test_split() {
        assert_eq!(
            ShellKind::Posix.split("cmd arg1 arg2").unwrap(),
            vec!["cmd", "arg1", "arg2"]
        );
        assert_eq!(
            ShellKind::Posix.split("cmd 'arg with space'").unwrap(),
            vec!["cmd", "arg with space"]
        );
    }

    #[test]
    fn test_activate_keyword() {
        assert_eq!(ShellKind::Posix.activate_keyword(), "source");
        assert_eq!(ShellKind::PowerShell.activate_keyword(), ".");
        assert_eq!(ShellKind::Cmd.activate_keyword(), "");
        assert_eq!(ShellKind::Nushell.activate_keyword(), "overlay use");
    }

    #[test]
    fn test_clear_screen() {
        assert_eq!(ShellKind::Cmd.clear_screen_command(), "cls");
        assert_eq!(ShellKind::Posix.clear_screen_command(), "clear");
    }
}
