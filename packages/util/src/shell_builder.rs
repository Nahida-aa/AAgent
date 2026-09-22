//! ShellBuilder — 把用户 task 请求翻译成 shell 可执行的程序 + 参数。
//!
//! 对齐 Zed `crates/util/src/shell_builder.rs`。
//! 供 task crate（SpawnInTerminal）和 future PTY backend 调用。

use std::borrow::Cow;

use crate::shell::{Shell, ShellKind, get_system_shell};

/// ShellBuilder is used to turn a user-requested task into a
/// program that can be executed by the shell.
pub struct ShellBuilder {
    /// The shell program to run (e.g. `bash`, `powershell`, `cmd.exe`).
    program: String,
    /// Extra args to pass to the shell before the `-c` / `-C` / `/C` command.
    args: Vec<String>,
    /// Whether to run the shell interactively (`-i` flag for POSIX/Fish/Nushell).
    interactive: bool,
    /// Whether to redirect stdin to /dev/null for the spawned command.
    redirect_stdin: bool,
    /// Detected shell kind — drives quoting, command separator, variable syntax.
    kind: ShellKind,
}

impl ShellBuilder {
    /// Create a new ShellBuilder as configured.
    pub fn new(shell: &Shell, is_windows: bool) -> Self {
        let (program, args) = match shell {
            Shell::System => (get_system_shell(), Vec::new()),
            Shell::Program(shell) => (shell.clone(), Vec::new()),
            Shell::WithArguments { program, args, .. } => (program.clone(), args.clone()),
        };

        let kind = ShellKind::new(&program, is_windows);
        Self {
            program,
            args,
            interactive: true,
            kind,
            redirect_stdin: false,
        }
    }

    /// Headless hosts (e.g. eval CLI) should use this — no `-i` flag.
    pub fn non_interactive(mut self) -> Self {
        self.interactive = false;
        self
    }

    /// Redirect stdin so the spawned command doesn't get stuck reading from it.
    pub fn redirect_stdin_to_dev_null(mut self) -> Self {
        self.redirect_stdin = true;
        self
    }

    /// Returns the label to show in the terminal tab.
    pub fn command_label(&self, command_to_use_in_label: &str) -> String {
        if command_to_use_in_label.trim().is_empty() {
            self.program.clone()
        } else {
            match self.kind {
                ShellKind::PowerShell | ShellKind::Pwsh => {
                    format!("{} -C '{}'", self.program, command_to_use_in_label)
                }
                ShellKind::Cmd => {
                    format!("{} /C \"{}\"", self.program, command_to_use_in_label)
                }
                ShellKind::Posix
                | ShellKind::Nushell
                | ShellKind::Fish
                | ShellKind::Csh
                | ShellKind::Tcsh
                | ShellKind::Rc
                | ShellKind::Xonsh
                | ShellKind::Elvish => {
                    let interactivity = self.interactive.then_some("-i ").unwrap_or_default();
                    format!(
                        "{PROGRAM} {interactivity}-c '{command_to_use_in_label}'",
                        PROGRAM = self.program
                    )
                }
            }
        }
    }

    /// Returns the program and arguments to run this task in a shell.
    ///
    /// 这是 terminal-provider / PTY backend 应该调用的入口。
    pub fn build(
        mut self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        if let Some(task_command) = task_command {
            // 有 args 时才需要 command prefix aware quoting
            let task_command = if !task_args.is_empty() {
                match self.kind.try_quote_prefix_aware(&task_command) {
                    Some(task_command) => task_command.into_owned(),
                    None => task_command,
                }
            } else {
                task_command
            };

            // 拼 combined_command：command + 空格分隔的 quoted args
            let mut combined_command = task_args.iter().fold(task_command, |mut command, arg| {
                command.push(' ');
                let shell_variable = self.kind.to_shell_variable(arg);
                command.push_str(&match self.kind.try_quote(&shell_variable) {
                    Some(shell_variable) => shell_variable,
                    None => Cow::Owned(shell_variable),
                });
                command
            });

            // stdin 重定向（如果配置了）
            if self.redirect_stdin {
                match self.kind {
                    ShellKind::Posix => {
                        // 先重定向，让语法错误也能工作
                        combined_command.insert_str(0, "exec </dev/null\n");
                    }
                    ShellKind::Fish => {
                        combined_command.insert_str(0, "begin; ");
                        combined_command.push_str("; end </dev/null");
                    }
                    ShellKind::Nushell
                    | ShellKind::Csh
                    | ShellKind::Tcsh
                    | ShellKind::Rc
                    | ShellKind::Xonsh
                    | ShellKind::Elvish => {
                        combined_command.insert(0, '(');
                        combined_command.push_str("\n) </dev/null");
                    }
                    ShellKind::PowerShell | ShellKind::Pwsh => {
                        combined_command.insert_str(0, "$null | & {");
                        combined_command.push_str("}");
                    }
                    ShellKind::Cmd => {
                        combined_command.push_str("< NUL");
                    }
                }
            }

            // shell 参数：-i / -c / -C / /C 等
            self.args
                .extend(self.kind.args_for_shell(self.interactive, combined_command));
        }

        (self.program, self.args)
    }

    /// 不做 quoting 的 build — Zed 说"should not exist but task infra broken"。
    #[doc(hidden)]
    pub fn build_no_quote(
        mut self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        if let Some(task_command) = task_command {
            let mut combined_command = task_args.iter().fold(task_command, |mut command, arg| {
                command.push(' ');
                command.push_str(&self.kind.to_shell_variable(arg));
                command
            });

            if self.redirect_stdin {
                match self.kind {
                    ShellKind::Posix => {
                        combined_command.insert_str(0, "exec </dev/null\n");
                    }
                    ShellKind::Fish => {
                        combined_command.insert_str(0, "begin; ");
                        combined_command.push_str("; end </dev/null");
                    }
                    ShellKind::Nushell
                    | ShellKind::Csh
                    | ShellKind::Tcsh
                    | ShellKind::Rc
                    | ShellKind::Xonsh
                    | ShellKind::Elvish => {
                        combined_command.insert(0, '(');
                        combined_command.push_str("\n) </dev/null");
                    }
                    ShellKind::PowerShell | ShellKind::Pwsh => {
                        combined_command.insert_str(0, "$null | & {");
                        combined_command.push_str("}");
                    }
                    ShellKind::Cmd => {
                        combined_command.push_str("< NUL");
                    }
                }
            }

            self.args
                .extend(self.kind.args_for_shell(self.interactive, combined_command));
        }

        (self.program, self.args)
    }

    /// Builds a `smol::process::Command` with the given task command and arguments.
    ///
    /// Prefer this over manually constructing a command with the output of `Self::build`,
    /// as this method handles `cmd` weirdness on windows correctly.
    pub fn build_smol_command(
        self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> smol::process::Command {
        smol::process::Command::from(self.build_std_command(task_command, task_args))
    }

    /// Builds a `std::process::Command` with the given task command and arguments.
    ///
    /// Prefer this over manually constructing a command with the output of `Self::build`,
    /// as this method handles `cmd` weirdness on windows correctly.
    pub fn build_std_command(
        self,
        mut task_command: Option<String>,
        task_args: &[String],
    ) -> std::process::Command {
        #[cfg(windows)]
        let kind = self.kind;
        if task_args.is_empty() {
            task_command = task_command
                .as_ref()
                .map(|cmd| self.kind.try_quote_prefix_aware(&cmd).map(Cow::into_owned))
                .unwrap_or(task_command);
        }
        let (program, args) = self.build(task_command, task_args);

        let mut child = crate::command::new_std_command(program);

        #[cfg(windows)]
        if kind == ShellKind::Cmd {
            use std::os::windows::process::CommandExt;

            for arg in args {
                child.raw_arg(arg);
            }
        } else {
            child.args(args);
        }

        #[cfg(not(windows))]
        child.args(args);

        child
    }

    /// Detected ShellKind — 供外部查询 shell 类型。
    pub fn kind(&self) -> ShellKind { self.kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nu_shell_variable_substitution() {
        let shell = Shell::Program("nu".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(
            Some("echo".into()),
            &[
                "${hello}".to_string(),
                "$world".to_string(),
                "nothing".to_string(),
                "--$something".to_string(),
                "$".to_string(),
                "${test".to_string(),
            ],
        );

        assert_eq!(program, "nu");
        assert_eq!(
            args,
            vec![
                "-i",
                "-c",
                "echo '$env.hello' '$env.world' nothing '--($env.something)' '$' '${test'"
            ]
        );
    }

    #[test]
    fn test_redirect_stdin_to_dev_null_precedence() {
        let shell = Shell::Program("nu".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some("echo".into()), &["nothing".to_string()]);

        assert_eq!(program, "nu");
        assert_eq!(args, vec!["-i", "-c", "(echo nothing\n) </dev/null"]);
    }

    #[test]
    fn test_redirect_stdin_to_dev_null_fish() {
        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some("echo".into()), &["test".to_string()]);

        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "begin; echo test; end </dev/null"]);
    }

    #[test]
    fn test_redirect_stdin_to_dev_null_preserves_heredoc() {
        let shell = Shell::Program("sh".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let command = "cat <<EOF\nhello\nEOF";
        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some(command.into()), &[]);

        assert_eq!(program, "sh");
        assert_eq!(
            args,
            vec!["-i", "-c", "exec </dev/null\ncat <<EOF\nhello\nEOF"]
        );
    }

    #[test]
    fn test_non_interactive_omits_interactive_flag() {
        let shell = Shell::Program("sh".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false).non_interactive();

        let (program, args) = shell_builder.build(Some("echo hello".into()), &[]);

        assert_eq!(program, "sh");
        assert_eq!(args, vec!["-c", "echo hello"]);
        assert!(
            !args.iter().any(|arg| arg == "-i"),
            "non-interactive shell command must not include `-i`"
        );
    }

    #[test]
    fn test_does_not_quote_sole_command_only() {
        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(Some("echo".into()), &[]);
        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "echo"]);

        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(Some("echo oo".into()), &[]);
        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "echo oo"]);
    }

    #[test]
    fn test_command_label() {
        let shell = Shell::Program("bash".to_owned());
        let builder = ShellBuilder::new(&shell, false);
        assert_eq!(builder.command_label("echo hi"), "bash -i -c 'echo hi'");
        assert_eq!(
            builder.non_interactive().command_label("echo hi"),
            "bash -c 'echo hi'"
        );

        let ps = Shell::Program("powershell".to_owned());
        let builder = ShellBuilder::new(&ps, true);
        assert_eq!(
            builder.command_label("Write-Host hi"),
            "powershell -C 'Write-Host hi'"
        );

        let cmd = Shell::Program("cmd.exe".to_owned());
        let builder = ShellBuilder::new(&cmd, true);
        assert_eq!(builder.command_label("dir"), "cmd.exe /C \"dir\"");
    }
}
