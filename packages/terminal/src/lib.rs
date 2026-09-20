//! AA Terminal Backend — 纯 PTY + alacritty 模拟器，零 GPUI 依赖。
//!
//! 渲染层（Element + View）在 `aa-terminal-view` crate.

pub mod alacritty;
mod builder;
mod cmd;
mod events;
mod task;
pub mod terminal_settings;
use async_channel::{Receiver, Sender};
use std::path::PathBuf;
use std::{process::ExitStatus, sync::Arc};
mod ansi_text;
mod colors;
mod pty_info;
pub use alacritty::{AlacrittyBackend, DisplayCell, DisplayCursor};
mod error;
mod model;
mod process_info;
mod shell;
use crate::{cmd::CwdHistoryEntry, task::TaskState};
pub use ansi_text::{AnsiSpans, ParsedAnsiText, parse_ansi_text, strip_ansi_text};
pub use colors::get_color_at_index;
pub use error::TerminalError;
pub use events::{Event, PtyEvent};
pub use model::Range;
pub use model::TerminalBounds;
mod subprocess;
/// Separates retained PTY process metadata from resources needed only while
/// the terminal is live.
enum PtyResources {
    Active(PtySender),
    Released,
}

enum TerminalType {
    Pty {
        resources: PtyResources,
        info: Arc<PtyProcessInfo>,
    },
    DisplayOnly,
}

pub struct Terminal {
    terminal_type: TerminalType,
    /// Set for non-PTY terminals (see [`HeadlessTerminal`]); owns the spawned
    /// subprocess and the task pumping its output into the grid.
    subprocess: Option<SubprocessHandle>,
    completion_tx: Option<Sender<Option<ExitStatus>>>,
    term: Arc<AlacrittyTermLock>,
    term_config: AlacrittyTermConfig,
    output_processor: Processor<StdSyncHandler>,
    events: VecDeque<InternalEvent>,
    /// This is only used for mouse mode cell change detection
    last_mouse: Option<(Point, SelectionSide)>,
    /// Window-relative position of the most recent left mouse-down. Used to
    /// apply a drag threshold before starting a selection (see #58970).
    mouse_down_position: Option<GpuiPoint<Pixels>>,
    pub matches: Vec<Range>,
    pub last_content: Content,
    pub selection_head: Option<Point>,

    pub breadcrumb_text: String,
    title_override: Option<String>,
    scroll_px: Pixels,
    next_link_id: usize,
    selection_phase: SelectionPhase,
    hyperlink_regex_searches: RegexSearches,
    task: Option<TaskState>,
    vi_mode_enabled: bool,
    is_remote_terminal: bool,
    last_mouse_move_time: Instant,
    last_hyperlink_search_position: Option<GpuiPoint<Pixels>>,
    mouse_down_hyperlink: Option<HyperlinkMatch>,
    #[cfg(windows)]
    shell_program: Option<String>,
    template: CopyTemplate,
    activation_script: Vec<String>,
    child_exited: Option<ExitStatus>,
    keyboard_input_sent: bool,
    init_command_startup_marker: Option<String>,
    init_command_startup_tx: Option<Sender<()>>,
    event_loop_task: Task<Result<(), anyhow::Error>>,
    background_executor: BackgroundExecutor,
    path_style: PathStyle,
    cwd_history: Vec<CwdHistoryEntry>,
    pending_cwd_boundary: Option<i32>,
    #[cfg(any(test, feature = "test-support"))]
    input_log: Vec<Vec<u8>>,
    #[cfg(test)]
    suppress_hyperlink_throttle_once: bool,
    #[cfg(any(test, feature = "test-support"))]
    pty_write_log: std::cell::RefCell<Vec<Vec<u8>>>,
}
impl Terminal {
    pub(crate) fn record_cwd_change(&mut self, new_working_directory: PathBuf) {
        if self.is_remote_terminal {
            return;
        }

        let scrollback_position = self.pending_cwd_boundary.take().unwrap_or_else(|| {
            let term = self.term.lock_unfair();
            Self::scrollback_position(term.grid().cursor.point.line.0, term.history_size())
        });
        self.cwd_history.push(CwdHistoryEntry {
            scrollback_position,
            working_directory: new_working_directory,
        });
    }
    fn scrollback_position(line: i32, history_size: usize) -> i32 {
        let history_size = i32::try_from(history_size).unwrap_or(i32::MAX);
        history_size.saturating_add(line)
    }
}
#[derive(PartialEq, Eq)]
enum SelectionPhase {
    Selecting,
    Ended,
}
