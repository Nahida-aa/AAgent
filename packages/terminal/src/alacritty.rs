use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use alacritty_terminal::event::{Event as AlacTermEvent, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions as _;
use alacritty_terminal::grid::{self};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::tty::{self, Options, Shell};
use alacritty_terminal::vte::ansi::{Color, CursorShape};
use gpui::{Pixels, px};
use parking_lot::Mutex;

/// Dimensions handed to the alacritty emulator; drives rows/cols of the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalBounds {
    pub cell_width: Pixels,
    pub line_height: Pixels,
    pub width: Pixels,
    pub height: Pixels,
    pub font_size: Pixels,
}

impl TerminalBounds {
    pub fn num_lines(&self) -> usize {
        if self.line_height <= px(0.0) {
            return 1;
        }
        (self.height / self.line_height).floor() as usize
    }

    pub fn num_columns(&self) -> usize {
        if self.cell_width <= px(0.0) {
            return 1;
        }
        (self.width / self.cell_width).floor() as usize
    }

    pub fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.num_lines() as u16,
            num_cols: self.num_columns() as u16,
            cell_width: f32::from(self.cell_width) as u16,
            cell_height: f32::from(self.line_height) as u16,
        }
    }
}

impl grid::Dimensions for TerminalBounds {
    fn total_lines(&self) -> usize {
        self.num_lines()
    }

    fn screen_lines(&self) -> usize {
        self.num_lines()
    }

    fn columns(&self) -> usize {
        self.num_columns()
    }
}

/// A snapshot of one visible cell, directly paintable by the GPUI element.
#[derive(Clone, Copy, Debug)]
pub struct DisplayCell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: Flags,
}

/// Forwards alacritty events through a bounded-ish queue; the element drains
/// this each frame and requests a redraw when output (Wakeup) arrived.
#[derive(Clone)]
struct Listener {
    events: Arc<Mutex<Vec<AlacTermEvent>>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: AlacTermEvent) {
        self.events.lock().push(event);
    }
}

/// The alacritty emulator + PTY behind a session.
pub struct AlacrittyBackend {
    term: Arc<FairMutex<Term<Listener>>>,
    pty_tx: EventLoopSender,
    events: Arc<Mutex<Vec<AlacTermEvent>>>,
    metrics: Arc<Mutex<TerminalMetrics>>,
}

/// Cached display metrics so `set_bounds(&self)` can mutate without `&mut`.
struct TerminalMetrics {
    cell_width: Pixels,
    line_height: Pixels,
    font_size: Pixels,
}

impl AlacrittyBackend {
    pub fn new(
        bounds: TerminalBounds,
        shell: Option<String>,
        working_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener = Listener {
            events: events.clone(),
        };

        let config = {
            let mut c = Config::default();
            c.scrolling_history = 10_000;
            c
        };

        let term = Arc::new(FairMutex::new(Term::new(config, &bounds, listener.clone())));
        let window_size = bounds.window_size();

        let shell =
            shell.unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));

        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());

        let pty = tty::new(
            &Options {
                shell: Some(Shell::new(shell, Default::default())),
                working_directory: Some(working_dir),
                drain_on_exit: false,
                env,
                child_signal_mask: None,
                #[cfg(target_os = "windows")]
                escape_args: false,
            },
            window_size,
            0,
        )?;

        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false)?;
        let pty_tx = event_loop.channel();
        event_loop.spawn();

        Ok(Self {
            term,
            pty_tx,
            events,
            metrics: Arc::new(Mutex::new(TerminalMetrics {
                cell_width: bounds.cell_width,
                line_height: bounds.line_height,
                font_size: bounds.font_size,
            })),
        })
    }

    pub fn bounds(&self) -> TerminalBounds {
        let term = self.term.lock();
        let m = self.metrics.lock();
        TerminalBounds {
            cell_width: m.cell_width,
            line_height: m.line_height,
            width: px(term.grid().columns() as f32 * f32::from(m.cell_width)),
            height: px(term.grid().screen_lines() as f32 * f32::from(m.line_height)),
            font_size: m.font_size,
        }
    }

    pub fn set_bounds(&self, bounds: TerminalBounds) {
        *self.metrics.lock() = TerminalMetrics {
            cell_width: bounds.cell_width,
            line_height: bounds.line_height,
            font_size: bounds.font_size,
        };
        self.term.lock().resize(bounds);
        let _ = self.pty_tx.send(Msg::Resize(bounds.window_size()));
    }

    pub fn has_events(&self) -> bool {
        !self.events.lock().is_empty()
    }

    pub fn drain_events(&self) {
        self.events.lock().clear();
    }

    pub fn write_input(&self, bytes: &[u8]) {
        let _ = self.pty_tx.send(Msg::Input(bytes.to_vec().into()));
    }

    /// Snapshot the visible grid (in display order, top to bottom) as cells.
    pub fn read_cells(&self) -> (Vec<DisplayCell>, usize, usize) {
        let term = self.term.lock();
        let rows = term.grid().screen_lines();
        let cols = term.grid().columns();
        let content = term.renderable_content();
        let mut cells = Vec::with_capacity(rows * cols);
        for indexed in content.display_iter {
            cells.push(cell_to_display(&indexed.cell));
        }
        (cells, rows, cols)
    }

    /// Snapshot the cursor position/shape in display coordinates.
    ///
    /// The grid reports the cursor's line as an offset from the top of the
    /// whole buffer; adding the scrollback `display_offset` yields the on-screen
    /// row (see zed's `DisplayCursor::from`).
    pub fn read_cursor(&self) -> Option<DisplayCursor> {
        let term = self.term.lock();
        let content = term.renderable_content();
        let display_row = content.cursor.point.line.0 + content.display_offset as i32;
        let rows = term.grid().screen_lines() as i32;
        if content.cursor.shape == CursorShape::Hidden || display_row < 0 || display_row >= rows {
            return None;
        }
        Some(DisplayCursor {
            row: display_row,
            col: content.cursor.point.column.0,
            shape: content.cursor.shape,
        })
    }
}

/// Cursor position in display coordinates, plus its shape.
#[derive(Clone, Copy, Debug)]
pub struct DisplayCursor {
    pub row: i32,
    pub col: usize,
    pub shape: CursorShape,
}

fn cell_to_display(cell: &Cell) -> DisplayCell {
    let c = cell.c;
    let (fg, bg) = if cell.flags.contains(Flags::INVERSE) {
        (cell.bg, cell.fg)
    } else {
        (cell.fg, cell.bg)
    };
    DisplayCell {
        c,
        fg,
        bg,
        flags: cell.flags,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;
    use std::time::Duration;

    fn test_bounds() -> TerminalBounds {
        TerminalBounds {
            cell_width: px(10.0),
            line_height: px(16.0),
            width: px(800.0),
            height: px(600.0),
            font_size: px(15.0),
        }
    }

    #[test]
    fn pty_spawns_and_renders_shell_output() {
        let backend = AlacrittyBackend::new(
            test_bounds(),
            Some("/bin/sh".into()),
            std::env::current_dir().unwrap(),
        )
        .expect("pty spawn");

        backend.write_input(b"printf 'aaBot-smoke\\n'\r");
        std::thread::sleep(Duration::from_millis(500));

        let (cells, rows, cols) = backend.read_cells();
        assert!(rows > 0 && cols > 0, "grid has dimensions");
        let joined: String = cells
            .iter()
            .map(|c| c.c)
            .collect::<Vec<char>>()
            .iter()
            .filter(|c| !c.is_control())
            .collect();
        let text = joined.replace('\0', " ").trim().to_string();
        eprintln!("terminal rows={rows} cols={cols} text={text:?}");

        let contains_smoke = text.contains("aaBot-smoke");
        // On some shells the echo-back differs; accept prompt presence too.
        assert!(
            contains_smoke,
            "expected echo output in grid, got: {text:?}"
        );
    }

    #[test]
    fn cursor_is_visible_after_shell_output() {
        let backend = AlacrittyBackend::new(
            test_bounds(),
            Some("/bin/sh".into()),
            std::env::current_dir().unwrap(),
        )
        .expect("pty spawn");

        backend.write_input(b"printf 'aaBot-smoke\\n'\r");
        std::thread::sleep(Duration::from_millis(500));

        let cursor = backend.read_cursor().expect("cursor should be visible");
        let (_, rows, _) = backend.read_cells();
        assert_eq!(
            cursor.shape,
            alacritty_terminal::vte::ansi::CursorShape::Block,
            "default cursor style is block"
        );
        assert!(
            (0..rows as i32).contains(&cursor.row),
            "cursor row {cursor:?} should be on-screen ({rows} rows)"
        );
    }
}
