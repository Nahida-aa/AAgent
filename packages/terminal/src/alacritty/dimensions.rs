use alacritty_terminal::grid::Dimensions;

use crate::TerminalBounds;

impl Dimensions for TerminalBounds {
    fn total_lines(&self) -> usize { self.screen_lines() }
    fn screen_lines(&self) -> usize { self.num_lines() }
    fn columns(&self) -> usize { self.num_columns() }
}
