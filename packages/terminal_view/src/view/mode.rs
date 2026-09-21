#[derive(Default, Clone)]
pub enum TerminalMode {
    #[default]
    Standalone,
    Embedded {
        max_lines_when_unfocused: Option<usize>,
    },
}

#[derive(Clone)]
pub enum ContentMode {
    Scrollable,
    Inline {
        displayed_lines: usize,
        total_lines: usize,
    },
}

impl ContentMode {
    pub fn is_limited(&self) -> bool {
        match self {
            ContentMode::Scrollable => false,
            ContentMode::Inline {
                displayed_lines,
                total_lines,
            } => displayed_lines < total_lines,
        }
    }

    pub fn is_scrollable(&self) -> bool { matches!(self, ContentMode::Scrollable) }
}
