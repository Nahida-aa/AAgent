use std::sync::atomic::AtomicU64;

use util::shell::ShellKind;

pub(super) static NEXT_INIT_COMMAND_STARTUP_MARKER_ID: AtomicU64 = AtomicU64::new(1);

pub(super) const INIT_COMMAND_STARTUP_MARKER_PREFIX: &str = "__zed_init_command_ready_";
pub(super) const INIT_COMMAND_STARTUP_MARKER_SUFFIX: &str = "__";
pub(super) const INIT_COMMAND_STARTUP_MARKER_SEARCH_LINES: usize = 64;

pub(super) fn init_command_startup_marker(marker_id: u64) -> String {
    format!("{INIT_COMMAND_STARTUP_MARKER_PREFIX}{marker_id}{INIT_COMMAND_STARTUP_MARKER_SUFFIX}")
}

pub(super) fn init_command_startup_marker_command(shell_kind: ShellKind, marker_id: u64) -> String {
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
