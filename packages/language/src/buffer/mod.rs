pub mod highlighted_text;

pub use highlighted_text::{HighlightedText, HighlightedTextBuilder};

/// Indicate whether a [`Buffer`] has permissions to edit.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Capability {
    /// The buffer is a mutable replica.
    ReadWrite,
    /// The buffer is a mutable replica, but toggled to be only readable.
    Read,
    /// The buffer is a read-only replica.
    ReadOnly,
}

impl Capability {
    /// Returns `true` if the capability is `ReadWrite`.
    pub fn editable(self) -> bool { matches!(self, Capability::ReadWrite) }
}

/// The shape of a selection cursor.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    /// A vertical bar
    #[default]
    Bar,
    /// A block that surrounds the following character
    Block,
    /// An underline that runs along the following character
    Underline,
    /// A box drawn around the following character
    Hollow,
}

impl From<settings::CursorShape> for CursorShape {
    fn from(shape: settings::CursorShape) -> Self {
        match shape {
            settings::CursorShape::Bar => CursorShape::Bar,
            settings::CursorShape::Block => CursorShape::Block,
            settings::CursorShape::Underline => CursorShape::Underline,
            settings::CursorShape::Hollow => CursorShape::Hollow,
        }
    }
}
