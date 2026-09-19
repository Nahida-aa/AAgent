//! 精简版文件系统抽象层。
//!
//! 对齐 Zed `crates/fs/src/fs.rs::Fs`，async_trait 抽象。
//! Project 持有 `Arc<dyn Fs>`，让整个系统可测试——测试时注入 FakeFs。

use std::path::{Path, PathBuf};

use anyhow::Result;

pub mod fake_fs;
pub mod real_fs;
pub mod watcher;

pub use fake_fs::{FakeFs, FakeFsEntry, FakeFsState, MTime};
pub use real_fs::RealFs;

// ---------- Fs trait ----------

/// 异步文件系统抽象。
///
/// 主线代码所有文件操作都走这个 trait。
/// 测试时注入 FakeFs，生产用 RealFs。
#[async_trait::async_trait]
pub trait Fs: Send + Sync {
    // ---- read ----
    async fn is_file(&self, path: &Path) -> bool;
    async fn is_dir(&self, path: &Path) -> bool;
    async fn path_exists(&self, path: &Path) -> bool;
    async fn load(&self, path: &Path) -> Result<String> {
        let bytes = self.load_bytes(path).await?;
        Ok(String::from_utf8(bytes)?)
    }
    async fn load_bytes(&self, path: &Path) -> Result<Vec<u8>>;

    // ---- write ----
    async fn create_dir(&self, path: &Path) -> Result<()>;
    async fn write(&self, path: &Path, content: &[u8]) -> Result<()>;
    async fn save(&self, path: &Path, text: &str) -> Result<()> {
        self.write(path, text.as_bytes()).await
    }
    async fn remove_file(&self, path: &Path) -> Result<()>;
    async fn remove_dir(&self, path: &Path, recursive: bool) -> Result<()>;
    async fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    // ---- meta ----
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf>;
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
    async fn metadata(&self, path: &Path) -> Result<Option<Metadata>>;
}

/// 文件/目录元信息（精简版，对齐 std::fs::Metadata）。
#[derive(Clone, Debug)]
pub struct Metadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub len: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_fs() {
        let fs = FakeFs::new_arc();
        assert!(!fs.path_exists(Path::new("/tmp/foo.txt")).await);
        fs.save(Path::new("/tmp/foo.txt"), "hello world")
            .await
            .unwrap();
        assert!(fs.path_exists(Path::new("/tmp/foo.txt")).await);
        assert!(fs.is_file(Path::new("/tmp/foo.txt")).await);
        assert!(!fs.is_dir(Path::new("/tmp/foo.txt")).await);
        assert_eq!(
            fs.load(Path::new("/tmp/foo.txt")).await.unwrap(),
            "hello world"
        );

        fs.create_dir(Path::new("/tmp/sub")).await.unwrap();
        assert!(fs.is_dir(Path::new("/tmp/sub")).await);
        fs.write(Path::new("/tmp/sub/bar.rs"), b"fn main() {}")
            .await
            .unwrap();

        let dirs = fs.read_dir(Path::new("/tmp")).await.unwrap();
        assert_eq!(dirs.len(), 2); // foo.txt + sub

        let meta = fs
            .metadata(Path::new("/tmp/foo.txt"))
            .await
            .unwrap()
            .unwrap();
        assert!(meta.is_file);
        assert_eq!(meta.len, 11);

        fs.remove_file(Path::new("/tmp/foo.txt")).await.unwrap();
        assert!(!fs.path_exists(Path::new("/tmp/foo.txt")).await);

        fs.remove_dir(Path::new("/tmp/sub"), true).await.unwrap();
        assert!(!fs.path_exists(Path::new("/tmp/sub")).await);
    }

    #[tokio::test]
    async fn test_rename() {
        let fs = FakeFs::new_arc();
        fs.save(Path::new("/a.txt"), "hello").await.unwrap();
        fs.rename(Path::new("/a.txt"), Path::new("/b.txt"))
            .await
            .unwrap();
        assert_eq!(fs.load(Path::new("/b.txt")).await.unwrap(), "hello");
        assert!(!fs.path_exists(Path::new("/a.txt")).await);
    }
}
