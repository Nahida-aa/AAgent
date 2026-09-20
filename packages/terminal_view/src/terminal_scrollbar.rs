use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{Bounds, Pixels, Point, point, px, size};
use terminal::Terminal;

#[derive(Debug)]
struct ScrollHandleState {
    line_height: Pixels,
    total_lines: usize,
    viewport_lines: usize,
    display_offset: usize,
}

impl ScrollHandleState {
    fn new(terminal: &Terminal) -> Self {
        Self {
            line_height: terminal.last_content().terminal_bounds.line_height,
            total_lines: terminal.total_lines(),
            viewport_lines: terminal.viewport_lines(),
            display_offset: terminal.last_content().display_offset,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalScrollHandle {
    state: Rc<RefCell<ScrollHandleState>>,
    pub future_display_offset: Rc<Cell<Option<usize>>>,
}

impl TerminalScrollHandle {
    pub fn new(terminal: &Terminal) -> Self {
        Self {
            state: Rc::new(RefCell::new(ScrollHandleState::new(terminal))),
            future_display_offset: Rc::new(Cell::new(None)),
        }
    }

    pub fn update(&self, terminal: &Terminal) {
        *self.state.borrow_mut() = ScrollHandleState::new(terminal);
    }
}

// ScrollableHandle trait 暂未实现（aa_gpui_kit_ui 里没有）
