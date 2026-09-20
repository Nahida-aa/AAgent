use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CwdHistoryEntry {
    /// Line offset in the retained scrollback buffer.
    scrollback_position: i32,
    working_directory: PathBuf,
}
