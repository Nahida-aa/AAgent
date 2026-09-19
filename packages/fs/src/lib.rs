//! 精简版文件系统抽象层。
//!
//! 对齐 Zed `crates/fs/src/fs.rs::Fs`，async_trait 抽象。
//! Project 持有 `Arc<dyn Fs>`，让整个系统可测试——测试时注入 MockFs。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;

// ---------- Fs trait ----------

/// 异步文件系统抽象。
///
/// 主线代码所有文件操作都走这个 trait。
/// 测试时注入 MockFs，生产用 RealFs。
#[async_trait::async_trait]
pub trait Fs: Send + Sync {
    // ---- read ----
    async fn is_file(&self, path: &Path) -> bool;
    async fn is_dir(&self, path: &Path) -> bool;
    async fn path_exists(&self, path: &Path) -> bool;
    async fn load(&self, path: &Path) -> Result<String>;
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

// ---------- RealFs ----------

/// 真实 OS 文件系统实现——生产环境用。
pub struct RealFs;

impl RealFs {
    pub fn new() -> Arc<dyn Fs> {
        Arc::new(Self)
    }
}

impl Default for RealFs {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Fs for RealFs {
    async fn is_file(&self, path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }

    async fn is_dir(&self, path: &Path) -> bool {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }

    async fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    async fn load(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }

    async fn load_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(std::fs::read(path)?)
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        Ok(std::fs::create_dir_all(path)?)
    }

    async fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(std::fs::write(path, content)?)
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        Ok(std::fs::remove_file(path)?)
    }

    async fn remove_dir(&self, path: &Path, recursive: bool) -> Result<()> {
        if recursive {
            Ok(std::fs::remove_dir_all(path)?)
        } else {
            Ok(std::fs::remove_dir(path)?)
        }
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        Ok(std::fs::rename(from, to)?)
    }

    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        Ok(std::fs::canonicalize(path)?)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)? {
            out.push(entry?.path());
        }
        Ok(out)
    }

    async fn metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(Metadata {
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            len: meta.len(),
        }))
    }
}

// ---------- MockFs (for tests) ----------

/// 内存里的文件系统——测试时用。
#[derive(Default)]
pub struct MockFs {
    files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    dirs: Mutex<HashMap<PathBuf, ()>>,
}

impl MockFs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_arc() -> Arc<dyn Fs> {
        Arc::new(Self::new())
    }
}

#[async_trait::async_trait]
impl Fs for MockFs {
    async fn is_file(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    async fn is_dir(&self, path: &Path) -> bool {
        self.dirs.lock().unwrap().contains_key(path)
    }

    async fn path_exists(&self, path: &Path) -> bool {
        let files = self.files.lock().unwrap();
        let dirs = self.dirs.lock().unwrap();
        files.contains_key(path) || dirs.contains_key(path)
    }

    async fn load(&self, path: &Path) -> Result<String> {
        let bytes = self
            .files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))?;
        Ok(String::from_utf8(bytes)?)
    }

    async fn load_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found").into())
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        // 递归创建父目录
        let mut dirs = self.dirs.lock().unwrap();
        if !path.as_os_str().is_empty() {
            dirs.insert(path.to_path_buf(), ());
        }
        // 简化：不自动创建父目录（调用方负责）
        Ok(())
    }

    async fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            self.create_dir(parent).await?;
        }
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), content.to_vec());
        Ok(())
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        self.files.lock().unwrap().remove(path);
        Ok(())
    }

    async fn remove_dir(&self, path: &Path, recursive: bool) -> Result<()> {
        let mut dirs = self.dirs.lock().unwrap();
        if recursive {
            dirs.retain(|k, _| !k.starts_with(path));
            self.files
                .lock()
                .unwrap()
                .retain(|k, _| !k.starts_with(path));
        } else {
            dirs.remove(path);
        }
        Ok(())
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let mut files = self.files.lock().unwrap();
        if let Some(content) = files.remove(from) {
            files.insert(to.to_path_buf(), content);
        }
        Ok(())
    }

    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        Ok(path.to_path_buf()) // MockFs 不做 symlink
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.lock().unwrap();
        let dirs = self.dirs.lock().unwrap();
        let mut out = Vec::new();
        let prefix = path.to_path_buf();
        for k in files.keys() {
            if let Ok(rel) = k.strip_prefix(&prefix) {
                if rel.components().count() == 1 {
                    out.push(k.clone());
                }
            }
        }
        for k in dirs.keys() {
            if let Ok(rel) = k.strip_prefix(&prefix) {
                if rel.components().count() == 1 {
                    out.push(k.clone());
                }
            }
        }
        out.sort();
        Ok(out)
    }

    async fn metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let files = self.files.lock().unwrap();
        if let Some(content) = files.get(path) {
            return Ok(Some(Metadata {
                is_file: true,
                is_dir: false,
                len: content.len() as u64,
            }));
        }
        let dirs = self.dirs.lock().unwrap();
        if dirs.contains_key(path) {
            return Ok(Some(Metadata {
                is_file: false,
                is_dir: true,
                len: 0,
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_fs() {
        let fs = MockFs::new_arc();
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
        let fs = MockFs::new_arc();
        fs.save(Path::new("/a.txt"), "hello").await.unwrap();
        fs.rename(Path::new("/a.txt"), Path::new("/b.txt"))
            .await
            .unwrap();
        assert_eq!(fs.load(Path::new("/b.txt")).await.unwrap(), "hello");
        assert!(!fs.path_exists(Path::new("/a.txt")).await);
    }
}

pub mod watcher;
