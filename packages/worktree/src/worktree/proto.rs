use super::*;
use rpc::proto;

impl<'a> From<&'a Entry> for proto::Entry {
    fn from(entry: &'a Entry) -> Self {
        Self {
            id: entry.id.to_proto(),
            is_dir: entry.is_dir(),
            path: entry.path.as_ref().as_unix_str().to_owned(),
            inode: entry.inode,
            mtime: entry.mtime.map(|time| time.into()),
            is_ignored: entry.is_ignored,
            is_hidden: entry.is_hidden,
            is_external: entry.is_external,
            is_fifo: entry.is_fifo,
            size: Some(entry.size),
            canonical_path: entry
                .canonical_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            is_unloaded: entry.kind == EntryKind::UnloadedDir,
        }
    }
}

impl TryFrom<(&CharBag, &PathMatcher, proto::Entry)> for Entry {
    type Error = anyhow::Error;

    fn try_from(
        (root_char_bag, always_included, entry): (&CharBag, &PathMatcher, proto::Entry),
    ) -> Result<Self> {
        let kind = if entry.is_dir {
            if entry.is_unloaded {
                EntryKind::UnloadedDir
            } else {
                EntryKind::Dir
            }
        } else {
            EntryKind::File
        };

        let path = RelPath::from_unix_str(&entry.path)
            .context("invalid relative path in proto message")?;
        let char_bag = char_bag_for_path(*root_char_bag, &path);
        let is_always_included = always_included.is_match(&path);
        Ok(Entry {
            id: ProjectEntryId::from_proto(entry.id),
            kind,
            path: path.into(),
            inode: entry.inode,
            mtime: entry.mtime.map(|time| time.into()),
            size: entry.size.unwrap_or(0),
            canonical_path: entry
                .canonical_path
                .map(|path_string| Arc::from(PathBuf::from(path_string))),
            is_ignored: entry.is_ignored,
            is_hidden: entry.is_hidden,
            is_always_included,
            is_external: entry.is_external,
            is_private: false,
            char_bag,
            is_fifo: entry.is_fifo,
        })
    }
}
