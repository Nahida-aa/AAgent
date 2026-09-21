pub mod buffer;
mod language_registry;
pub use buffer::{
    Capability, CursorShape,
    highlighted_text::{HighlightedText, HighlightedTextBuilder},
};
pub use language_registry::LanguageRegistry;
