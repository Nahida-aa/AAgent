//! AA Terminal Backend — 纯 PTY + alacritty 模拟器，零 GPUI 依赖。
//!
//! 渲染层（Element + View）在 `aa-terminal-view` crate.

pub mod alacritty;
mod builder;
pub use builder::TerminalBuilder;
mod cmd;
mod events;
mod hyperlink;
mod input;
mod mouse;
mod task;
pub mod terminal_settings;
use async_channel::{Receiver, Sender};
use collections::HashMap;
use gpui::{App, BackgroundExecutor, Context, EventEmitter, Pixels, Point as GpuiPoint, Task, px};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::{process::ExitStatus, sync::Arc};
use util::paths::PathStyle;
use util::shell::Shell;
use vte::ansi::{Processor, StdSyncHandler};
mod actions;
mod ansi_text;
mod bounds;
mod cell;
mod colors;
mod cursor;
mod error;
mod headless;
mod hyperlink_handlers;
mod keyboard;
mod pty_info;
mod pty_io;
use terminal_settings::{AlternateScroll, CursorShape as SettingsCursorShape, TerminalSettings};
mod cwd;
mod event_loop;
mod mappings;
mod mode;
pub use mode::TerminalMode;
mod modes;
mod process_info;
mod pty_resources;
mod render;
mod scroll;
mod selection;
mod shell;
pub use shell::insert_zed_terminal_env;
mod startup_marker;
use crate::alacritty::{
    AlacrittyTermConfig, AlacrittyTermLock, HyperlinkMatch, PtySender, RegexSearches,
};
use crate::cursor::{Cursor, CursorShape, Point, Range, SelectionRange};
use crate::events::{InternalEvent, TerminalBackendEvent};
use crate::pty_info::{ProcessIdGetter, PtyProcessInfo};
use crate::subprocess::SubprocessHandle;
use crate::task::{TaskState, TaskStatus};
pub use ansi_text::{AnsiSpans, ParsedAnsiText, parse_ansi_text, strip_ansi_text};
pub use bounds::TerminalBounds;
pub use colors::get_color_at_index;
pub use error::TerminalError;
pub use events::{Event, PtyEvent};
mod subprocess;
use crate::mappings::colors::to_vte_rgb;
use crate::mappings::keys::to_esc_str;
use crate::modes::Modes;
mod foreground;

pub use crate::{
    cell::{Cell, Content, GridLinesChange, IndexedCell, RenderableCells},
    hyperlink::{Hyperlink, HyperlinkData},
    selection::{
        HoveredWord, Scroll, Search, Selection, SelectionPhase, SelectionSide, SelectionType,
        ViMotion,
    },
};
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
    pub(crate) terminal_type: TerminalType,
    /// Set for non-PTY terminals (see [`HeadlessTerminal`]); owns the spawned
    /// subprocess and the task pumping its output into the grid.
    pub(crate) subprocess: Option<SubprocessHandle>,
    pub(crate) completion_tx: Option<Sender<Option<ExitStatus>>>,
    pub(crate) term: Arc<AlacrittyTermLock>,
    pub(crate) term_config: AlacrittyTermConfig,
    pub(crate) output_processor: Processor<StdSyncHandler>,
    pub(crate) events: VecDeque<InternalEvent>,
    /// This is only used for mouse mode cell change detection
    pub(crate) last_mouse: Option<(Point, SelectionSide)>,
    /// Window-relative position of the most recent left mouse-down. Used to
    /// apply a drag threshold before starting a selection (see #58970).
    pub(crate) mouse_down_position: Option<GpuiPoint<Pixels>>,
    pub matches: Vec<Range>,
    pub last_content: Content,
    pub selection_head: Option<Point>,

    pub breadcrumb_text: String,
    pub(crate) title_override: Option<String>,
    pub(crate) scroll_px: Pixels,
    pub(crate) next_link_id: usize,
    pub(crate) selection_phase: SelectionPhase,
    pub(crate) hyperlink_regex_searches: RegexSearches,
    pub(crate) task: Option<TaskState>,
    pub(crate) vi_mode_enabled: bool,
    pub(crate) is_remote_terminal: bool,
    pub(crate) last_mouse_move_time: Instant,
    pub(crate) last_hyperlink_search_position: Option<GpuiPoint<Pixels>>,
    pub(crate) mouse_down_hyperlink: Option<HyperlinkMatch>,
    #[cfg(windows)]
    pub(crate) shell_program: Option<String>,
    pub(crate) template: CopyTemplate,
    pub(crate) activation_script: Vec<String>,
    pub(crate) child_exited: Option<ExitStatus>,
    pub(crate) keyboard_input_sent: bool,
    pub(crate) init_command_startup_marker: Option<String>,
    pub(crate) init_command_startup_tx: Option<Sender<()>>,
    pub(crate) event_loop_task: Task<Result<(), anyhow::Error>>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) path_style: PathStyle,
    pub(crate) cwd_history: Vec<CwdHistoryEntry>,
    pub(crate) pending_cwd_boundary: Option<i32>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) input_log: Vec<Vec<u8>>,
    #[cfg(test)]
    pub(crate) suppress_hyperlink_throttle_once: bool,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) pty_write_log: std::cell::RefCell<Vec<Vec<u8>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CwdHistoryEntry {
    /// Line offset in the retained scrollback buffer.
    scrollback_position: i32,
    working_directory: PathBuf,
}

pub(crate) struct CopyTemplate {
    shell: Shell,
    env: HashMap<String, String>,
    cursor_shape: SettingsCursorShape,
    alternate_scroll: AlternateScroll,
    max_scroll_history_lines: Option<usize>,
    path_hyperlink_regexes: Vec<String>,
    path_hyperlink_timeout: Duration,
    window_id: u64,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(subprocess) = self.subprocess.take() {
            subprocess.kill();
        }
        self.release_pty_resources();
    }
}

impl EventEmitter<Event> for Terminal {}
