#![cfg(any(test, feature = "test-support"))]

use super::*;

#[cfg(any(test, feature = "test-support"))]
pub struct TestFile {
    pub path: Arc<RelPath>,
    pub root_name: String,
    pub local_root: Option<PathBuf>,
}

#[cfg(any(test, feature = "test-support"))]
impl File for TestFile {
    fn path(&self) -> &Arc<RelPath> { &self.path }

    fn full_path(&self, _: &gpui::App) -> PathBuf {
        PathBuf::from(self.root_name.clone()).join(self.path.as_std_path())
    }

    fn as_local(&self) -> Option<&dyn LocalFile> {
        if self.local_root.is_some() {
            Some(self)
        } else {
            None
        }
    }

    fn disk_state(&self) -> DiskState { unimplemented!() }

    fn file_name<'a>(&'a self, _: &'a gpui::App) -> &'a str {
        self.path().file_name().unwrap_or(self.root_name.as_ref())
    }

    fn worktree_id(&self, _: &App) -> WorktreeId { WorktreeId::from_usize(0) }

    fn to_proto(&self, _: &App) -> rpc::proto::File { unimplemented!() }

    fn is_private(&self) -> bool { false }

    fn path_style(&self, _cx: &App) -> PathStyle { PathStyle::local() }
}

#[cfg(any(test, feature = "test-support"))]
impl LocalFile for TestFile {
    fn abs_path(&self, _cx: &App) -> PathBuf {
        PathBuf::from(self.local_root.as_ref().unwrap())
            .join(&self.root_name)
            .join(self.path.as_std_path())
    }

    fn load(&self, _cx: &App) -> Task<Result<String>> { unimplemented!() }

    fn load_bytes(&self, _cx: &App) -> Task<Result<Vec<u8>>> { unimplemented!() }
}
