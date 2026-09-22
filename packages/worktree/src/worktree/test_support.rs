#![cfg(feature = "test-support")]

use super::*;

impl CreatedEntry {
    pub fn into_included(self) -> Option<Entry> {
        match self {
            CreatedEntry::Included(entry) => Some(entry),
            CreatedEntry::Excluded { .. } => None,
        }
    }
}

pub(crate) async fn retouch_sentinel(fs: &dyn Fs, abs_path: &Path) {
    fs.create_file(
        abs_path,
        fs::CreateOptions {
            overwrite: true,
            ignore_if_exists: false,
        },
    )
    .await
    .unwrap();
}
pub(crate) async fn retouch_and_remove_sentinel(fs: &dyn Fs, abs_path: &Path) {
    retouch_sentinel(fs, abs_path).await;
    fs.remove_file(
        abs_path,
        RemoveOptions {
            recursive: false,
            ignore_if_not_exists: true,
        },
    )
    .await
    .unwrap();
}
