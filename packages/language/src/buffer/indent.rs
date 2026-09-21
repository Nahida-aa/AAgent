use super::*;

/// The kind and amount of indentation in a particular line. For now,
/// assumes that indentation is all the same character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct IndentSize {
    /// The number of bytes that comprise the indentation.
    pub len: u32,
    /// The kind of whitespace used for indentation.
    pub kind: IndentKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IndentKind {
    #[default]
    Space,
    Tab,
}

impl IndentSize {
    /// Returns an [`IndentSize`] representing the given spaces.
    pub fn spaces(len: u32) -> Self {
        Self {
            len,
            kind: IndentKind::Space,
        }
    }

    /// Returns an [`IndentSize`] representing a tab.
    pub fn tab() -> Self {
        Self {
            len: 1,
            kind: IndentKind::Tab,
        }
    }

    /// An iterator over the characters represented by this [`IndentSize`].
    pub fn chars(&self) -> impl Iterator<Item = char> {
        iter::repeat(self.char()).take(self.len as usize)
    }

    /// The character representation of this [`IndentSize`].
    pub fn char(&self) -> char {
        match self.kind {
            IndentKind::Space => ' ',
            IndentKind::Tab => '\t',
        }
    }

    /// Consumes the current [`IndentSize`] and returns a new one that has
    /// been shrunk or enlarged by the given size along the given direction.
    pub fn with_delta(mut self, direction: Ordering, size: IndentSize) -> Self {
        match direction {
            Ordering::Less => {
                if self.kind == size.kind && self.len >= size.len {
                    self.len -= size.len;
                }
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                if self.len == 0 {
                    self = size;
                } else if self.kind == size.kind {
                    self.len += size.len;
                }
            }
        }
        self
    }

    /// Returns the number of indentation characters to remove when outdenting to the
    /// previous editor tab stop.
    pub fn outdent_len(self, tab_size: NonZeroU32) -> u32 {
        if self.len == 0 {
            return 0;
        }

        match self.kind {
            IndentKind::Space => {
                let tab_size = tab_size.get();
                let columns_to_prev_tab_stop = self.len % tab_size;
                if columns_to_prev_tab_stop == 0 {
                    tab_size
                } else {
                    columns_to_prev_tab_stop
                }
            }
            IndentKind::Tab => 1,
        }
    }

    pub fn len_with_expanded_tabs(&self, tab_size: NonZeroU32) -> usize {
        match self.kind {
            IndentKind::Space => self.len as usize,
            IndentKind::Tab => self.len as usize * tab_size.get() as usize,
        }
    }
}

pub(super) fn indent_size_for_line(text: &text::BufferSnapshot, row: u32) -> IndentSize {
    indent_size_for_text(text.chars_at(Point::new(row, 0)))
}

pub(super) fn indent_size_for_text(text: impl Iterator<Item = char>) -> IndentSize {
    let mut result = IndentSize::spaces(0);
    for c in text {
        let kind = match c {
            ' ' => IndentKind::Space,
            '\t' => IndentKind::Tab,
            _ => break,
        };
        if result.len == 0 {
            result.kind = kind;
        }
        result.len += 1;
    }
    result
}
