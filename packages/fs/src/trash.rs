use std::ffi::OsString;
use std::path::PathBuf;

use slotmap::KeyData;

/// Represents a file or directory that has been moved to the system trash,
/// retaining enough information to restore it to its original location.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct TrashedEntry {
    /// Platform-specific identifier for the file/directory in the trash.
    ///
    /// * Freedesktop – Path to the `.trashinfo` file.
    /// * macOS & Windows – Full path to the file/directory in the system's
    /// trash.
    pub(crate) id: OsString,
    /// Name of the file/directory at the time of trashing, including extension.
    pub(crate) name: OsString,
    /// Absolute path to the parent directory at the time of trashing.
    pub(crate) original_parent: PathBuf,
}

impl From<trash::TrashItem> for TrashedEntry {
    fn from(item: trash::TrashItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            original_parent: item.original_parent,
        }
    }
}

impl TrashedEntry {
    pub(crate) fn into_trash_item(self) -> trash::TrashItem {
        trash::TrashItem {
            id: self.id,
            name: self.name,
            original_parent: self.original_parent,
            // `TrashedEntry` doesn't preserve `time_deleted` as we don't
            // currently need it for restore, so we default it to 0 here.
            time_deleted: 0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TrashRestoreError {
    #[error("The specified `path` ({}) was not found in the system's trash.", path.display())]
    NotFound { path: PathBuf },
    #[error("File or directory ({}) already exists at the restore destination.", path.display())]
    Collision { path: PathBuf },
    // This should never occur, the only way to get a TrashId is to undo
    // consumes the Change::Trashed. We worry about remoting duplicate messages
    // we do not want to crash the app then which is why this error is there.
    #[error("The item was already restored")]
    AlreadyRestored,
    #[error("Unknown error ({description})")]
    Unknown { description: String },
}

impl From<trash::Error> for TrashRestoreError {
    fn from(err: trash::Error) -> Self {
        match err {
            trash::Error::RestoreCollision { path, .. } => Self::Collision { path },
            trash::Error::Unknown { description } => Self::Unknown { description },
            other => Self::Unknown {
                description: other.to_string(),
            },
        }
    }
}

slotmap::new_key_type! { pub struct TrashId; }

impl TrashId {
    pub fn from_proto(value: u64) -> Self { KeyData::from_ffi(value).into() }

    pub fn to_proto(self) -> u64 { self.0.as_ffi() }
}
