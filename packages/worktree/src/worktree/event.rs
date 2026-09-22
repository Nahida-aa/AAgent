use super::*;

#[derive(Debug)]
pub enum CreatedEntry {
    Included(Entry),
    Excluded { abs_path: PathBuf },
}

#[derive(Debug)]
pub struct LoadedFile {
    pub file: Arc<File>,
    pub text: Rope,
    pub line_ending: LineEnding,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub is_writable: bool,
}

pub struct LoadedBinaryFile {
    pub file: Arc<File>,
    pub content: Vec<u8>,
}

impl fmt::Debug for LoadedBinaryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedBinaryFile")
            .field("file", &self.file)
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    UpdatedEntries(UpdatedEntriesSet),
    UpdatedGitRepositories(UpdatedGitRepositoriesSet),
    UpdatedRootRepoCommonDir { old: Option<Arc<SanitizedPath>> },
    DeletedEntry(ProjectEntryId),
    Deleted,
}
