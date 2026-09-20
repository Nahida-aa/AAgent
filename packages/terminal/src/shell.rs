use collections::{HashMap, VecDeque};
use gpui::App;
use util::ShellKind;
/// Process-wide flag set by headless hosts (e.g. the eval CLI) that have no
/// controlling TTY. In such sandboxes PTY allocation and acquiring a
/// controlling terminal fail with `ENOTTY`, so when this is set terminals run
/// their command as a plain subprocess with piped output instead of through a
/// PTY. The normal editor leaves it unset to preserve the interactive PTY
/// experience.
#[derive(Clone, Copy, Default)]
pub struct HeadlessTerminal(pub bool);

impl gpui::Global for HeadlessTerminal {}

impl HeadlessTerminal {
    pub fn is_enabled(cx: &App) -> bool {
        cx.try_global::<Self>().is_some_and(|headless| headless.0)
    }
}

/// Inserts Zed-specific environment variables for terminal sessions.
/// Used by both local terminals and remote terminals (via SSH).
pub fn insert_zed_terminal_env(
    env: &mut HashMap<String, String>,
    version: &impl std::fmt::Display,
) {
    env.insert("ZED_TERM".to_string(), "true".to_string());
    env.insert("TERM_PROGRAM".to_string(), "zed".to_string());
    env.insert("TERM".to_string(), "xterm-256color".to_string());
    env.insert("COLORTERM".to_string(), "truecolor".to_string());
    env.insert("TERM_PROGRAM_VERSION".to_string(), version.to_string());
}

const INIT_COMMAND_STARTUP_MARKER_PREFIX: &str = "__zed_init_command_ready_";
const INIT_COMMAND_STARTUP_MARKER_SUFFIX: &str = "__";
const INIT_COMMAND_STARTUP_MARKER_SEARCH_LINES: usize = 64;

fn init_command_startup_marker(marker_id: u64) -> String {
    format!("{INIT_COMMAND_STARTUP_MARKER_PREFIX}{marker_id}{INIT_COMMAND_STARTUP_MARKER_SUFFIX}")
}

fn init_command_startup_marker_command(shell_kind: ShellKind, marker_id: u64) -> String {
    // Split the marker across the command so its echo can't satisfy the
    // handshake; only the command's output contains the contiguous marker.
    match shell_kind {
        ShellKind::PowerShell | ShellKind::Pwsh => format!(
            "Write-Output ('{INIT_COMMAND_STARTUP_MARKER_PREFIX}' + '{marker_id}' + '{INIT_COMMAND_STARTUP_MARKER_SUFFIX}')"
        ),
        ShellKind::Cmd => {
            format!(
                "<nul set /p zed_init_ready={INIT_COMMAND_STARTUP_MARKER_PREFIX}&echo {marker_id}{INIT_COMMAND_STARTUP_MARKER_SUFFIX}"
            )
        }
        ShellKind::Nushell => {
            format!(
                "print $\"{INIT_COMMAND_STARTUP_MARKER_PREFIX}({marker_id}){INIT_COMMAND_STARTUP_MARKER_SUFFIX}\""
            )
        }
        ShellKind::Posix
        | ShellKind::Csh
        | ShellKind::Tcsh
        | ShellKind::Rc
        | ShellKind::Fish
        | ShellKind::Xonsh
        | ShellKind::Elvish => format!(
            "printf '%s%s%s\\n' {INIT_COMMAND_STARTUP_MARKER_PREFIX} {marker_id} {INIT_COMMAND_STARTUP_MARKER_SUFFIX}"
        ),
    }
}

#[derive(Default)]
struct ShellParams {
    program: String,
    args: Option<Vec<String>>,
    title_override: Option<String>,
}

impl ShellParams {
    fn new(program: String, args: Option<Vec<String>>, title_override: Option<String>) -> Self {
        log::debug!("Using {program} as shell");
        Self {
            program,
            args,
            title_override,
        }
    }
}
