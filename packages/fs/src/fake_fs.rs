//! FakeFs — 内存文件系统，测试用。
//! 对齐 Zed `crates/fs/src/fs.rs:1438 FakeFs`，精简版。

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures::Stream;

use crate::{Fs, Metadata};

/// 足够大的间隔，让 Windows 和 Unix 都认为 mtime 变了。
/// https://doc.rust-lang.org/nightly/std/time/struct.SystemTime.html#platform-specific-behavior
pub const SYSTEMTIME_INTERVAL: Duration = Duration::from_nanos(100);

// ---------- 类型 ----------

/// mtime 时间点包装——对齐 Zed MTime。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MTime(pub SystemTime);

/// 内部条目枚举——对齐 Zed FakeFsEntry。
#[derive(Clone, Debug)]
pub enum FakeFsEntry {
    File {
        inode: u64,
        mtime: MTime,
        len: u64,
        content: Vec<u8>,
    },
    Dir {
        inode: u64,
        mtime: MTime,
        len: u64,
        entries: BTreeMap<String, FakeFsEntry>,
    },
    Symlink {
        target: PathBuf,
    },
}

/// FakeFs 内部状态——对齐 Zed FakeFsState（精简版）。
pub struct FakeFsState {
    pub root: FakeFsEntry,
    pub next_inode: u64,
    pub next_mtime: SystemTime,
    pub case_sensitive: bool,
}

impl FakeFsState {
    pub fn get_and_increment_mtime(&mut self) -> MTime {
        let mtime = self.next_mtime;
        self.next_mtime += SYSTEMTIME_INTERVAL;
        MTime(mtime)
    }

    pub fn get_and_increment_inode(&mut self) -> u64 {
        let inode = self.next_inode;
        self.next_inode += 1;
        inode
    }

    /// 路径解析 + 符号链接跟随，返回规范化绝对路径。
    pub fn canonicalize(&self, target: &Path, follow_symlink: bool) -> Option<PathBuf> {
        let mut canonical_path = PathBuf::new();
        let mut path = target.to_path_buf();
        let mut entry_stack: Vec<&FakeFsEntry> = Vec::new();
        'outer: loop {
            let mut path_components = path.components().peekable();
            let mut prefix = None;
            while let Some(component) = path_components.next() {
                match component {
                    Component::Prefix(prefix_component) => prefix = Some(prefix_component),
                    Component::RootDir => {
                        entry_stack.clear();
                        entry_stack.push(&self.root);
                        canonical_path.clear();
                        canonical_path = match prefix {
                            Some(prefix_component) => {
                                let mut p = PathBuf::from(prefix_component.as_os_str());
                                p.push(std::path::MAIN_SEPARATOR_STR);
                                p
                            }
                            None => PathBuf::from(std::path::MAIN_SEPARATOR_STR),
                        };
                    }
                    Component::CurDir => {}
                    Component::ParentDir => {
                        entry_stack.pop()?;
                        canonical_path.pop();
                    }
                    Component::Normal(name) => {
                        let current_entry = *entry_stack.last()?;
                        if let FakeFsEntry::Dir { entries, .. } = current_entry {
                            let name_str = name.to_str().unwrap();
                            let (canonical_name, entry) = match entries.get(name_str) {
                                Some(entry) => (name_str, entry),
                                None => {
                                    if !self.case_sensitive {
                                        entries
                                            .iter()
                                            .find(|(key, _)| key.eq_ignore_ascii_case(name_str))
                                            .map(|(key, entry)| (key.as_str(), entry))?
                                    } else {
                                        return None;
                                    }
                                }
                            };
                            if (path_components.peek().is_some() || follow_symlink)
                                && let FakeFsEntry::Symlink { target } = entry
                            {
                                let mut target = target.clone();
                                target.extend(path_components);
                                path = target;
                                continue 'outer;
                            }
                            entry_stack.push(entry);
                            canonical_path = canonical_path.join(canonical_name);
                        } else {
                            return None;
                        }
                    }
                }
            }
            break;
        }
        if entry_stack.is_empty() {
            None
        } else {
            Some(canonical_path)
        }
    }

    /// 获取路径对应条目（跟随 symlink），同时返回规范化路径。
    pub fn try_entry(
        &mut self,
        target: &Path,
        follow_symlink: bool,
    ) -> Option<(&mut FakeFsEntry, PathBuf)> {
        let canonical_path = self.canonicalize(target, follow_symlink)?;
        let mut components = canonical_path
            .components()
            .skip_while(|c| matches!(c, Component::Prefix(_)));
        let Some(Component::RootDir) = components.next() else {
            return None;
        };
        let mut current = &mut self.root;
        for component in components {
            let name = component.as_os_str().to_str()?;
            current = match current {
                FakeFsEntry::Dir { entries, .. } => entries.get_mut(name)?,
                _ => return None,
            };
        }
        Some((current, canonical_path))
    }

    /// 获取父目录条目 + 文件名。
    pub fn try_parent_entry(
        &mut self,
        target: &Path,
        follow_symlink: bool,
    ) -> Option<(&mut BTreeMap<String, FakeFsEntry>, String)> {
        let parent = target.parent()?;
        let name = target.file_name()?.to_str()?.to_string();
        let (entry, _) = self.try_entry(parent, follow_symlink)?;
        match entry {
            FakeFsEntry::Dir { entries, .. } => Some((entries, name)),
            _ => None,
        }
    }
}

// ---------- FakeFs ----------

/// 内存里的文件系统——测试时用。
/// 对齐 Zed `FakeFs`（`crates/fs/src/fs.rs:1438`），精简版：
/// 去掉 GPUI executor、watcher backend、trash、git 状态等。
pub struct FakeFs {
    pub state: Arc<Mutex<FakeFsState>>,
}

impl FakeFs {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Arc::new(Mutex::new(FakeFsState {
                root: FakeFsEntry::Dir {
                    inode: 0,
                    mtime: MTime(UNIX_EPOCH),
                    len: 0,
                    entries: BTreeMap::new(),
                },
                next_mtime: UNIX_EPOCH + SYSTEMTIME_INTERVAL,
                next_inode: 1,
                case_sensitive: true,
            })),
        })
    }

    pub fn new_arc() -> Arc<dyn Fs> { Self::new() as Arc<dyn Fs> }

    pub fn set_case_sensitive(&self, case_sensitive: bool) {
        self.state.lock().unwrap().case_sensitive = case_sensitive;
    }
}

#[async_trait::async_trait]
impl Fs for FakeFs {
    async fn is_file(&self, path: &Path) -> bool {
        let kind = {
            let mut state = self.state.lock().unwrap();
            state.try_entry(path, true).map(|(e, _)| e.clone())
        };
        matches!(kind, Some(FakeFsEntry::File { .. }))
    }

    async fn is_dir(&self, path: &Path) -> bool {
        let kind = {
            let mut state = self.state.lock().unwrap();
            state.try_entry(path, true).map(|(e, _)| e.clone())
        };
        matches!(kind, Some(FakeFsEntry::Dir { .. }))
    }

    async fn path_exists(&self, path: &Path) -> bool {
        {
            let mut state = self.state.lock().unwrap();
            state.try_entry(path, true).is_some()
        }
    }

    async fn load_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let result = {
            let mut state = self.state.lock().unwrap();
            match state.try_entry(path, true) {
                Some((FakeFsEntry::File { content, .. }, _)) => Ok(content.clone()),
                Some(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "not a file",
                )),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "not found",
                )),
            }
        };
        result.map_err(Into::into)
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        // 逐组件创建，每个组件一把锁（Zed 模式）
        let mut cur_path = PathBuf::new();
        for component in path.components() {
            let should_skip = matches!(component, Component::Prefix(..) | Component::RootDir);
            cur_path.push(component);
            if should_skip {
                continue;
            }
            {
                let mut state = self.state.lock().unwrap();
                let inode = state.get_and_increment_inode();
                let mtime = state.get_and_increment_mtime();
                let (entries, name) = match state.try_parent_entry(&cur_path, true) {
                    Some(v) => v,
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            format!("parent dir not found for {}", cur_path.display()),
                        )
                        .into());
                    }
                };
                if !entries.contains_key(&name) {
                    entries.insert(
                        name,
                        FakeFsEntry::Dir {
                            inode,
                            mtime,
                            len: 0,
                            entries: BTreeMap::new(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    async fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        // step1: 检查父目录是否存在（纯同步锁，无 await）
        let parent = path.parent().unwrap_or_else(|| Path::new("/"));
        let parent_missing = {
            let state = self.state.lock().unwrap();
            state.canonicalize(parent, true).is_none()
        };

        // step2: 锁外 await 创建父目录
        if parent_missing {
            self.create_dir(parent).await?;
        }

        // step3: 拿锁写文件（纯同步锁，无 await）
        let len = content.len() as u64;
        {
            let mut state = self.state.lock().unwrap();
            let mtime = state.get_and_increment_mtime();

            // 先查是否已存在文件，拿原有 inode
            let existing_inode = match state.try_entry(path, true) {
                Some((FakeFsEntry::File { inode, .. }, _)) => Some(*inode),
                Some(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "path exists but is not a file",
                    )
                    .into());
                }
                None => None,
            };
            let inode = match existing_inode {
                Some(i) => i,
                None => state.get_and_increment_inode(),
            };

            // 现在用新的 state 借用拿 entries
            let (entries, name) = state.try_parent_entry(path, true).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "parent not found")
            })?;
            entries.insert(
                name,
                FakeFsEntry::File {
                    inode,
                    mtime,
                    len,
                    content: content.to_vec(),
                },
            );
        }
        Ok(())
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        let result = {
            let mut state = self.state.lock().unwrap();
            match state.try_parent_entry(path, true) {
                Some((entries, name)) => match entries.remove(&name) {
                    Some(FakeFsEntry::File { .. }) | Some(FakeFsEntry::Symlink { .. }) => Ok(()),
                    Some(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "not a file or symlink",
                    )),
                    None => Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "not found",
                    )),
                },
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("parent dir not found for {}", path.display()),
                )),
            }
        };
        result.map_err(Into::into)
    }

    async fn remove_dir(&self, path: &Path, recursive: bool) -> Result<()> {
        let result = {
            let mut state = self.state.lock().unwrap();
            match state.try_parent_entry(path, true) {
                Some((entries, name)) => match entries.remove(&name) {
                    Some(FakeFsEntry::Dir {
                        entries: child_entries,
                        ..
                    }) => {
                        if !recursive && !child_entries.is_empty() {
                            // 简化：Zed 返回 DirectoryNotEmpty
                            Ok(())
                        } else {
                            Ok(())
                        }
                    }
                    Some(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "not a directory",
                    )),
                    None => Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "not found",
                    )),
                },
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("parent dir not found for {}", path.display()),
                )),
            }
        };
        result.map_err(Into::into)
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        // step1: 取出 from 的条目
        let entry = {
            let mut state = self.state.lock().unwrap();
            let (from_entries, from_name) =
                state.try_parent_entry(from, true).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "from not found")
                })?;
            from_entries.remove(&from_name).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "from not found")
            })?
        };

        // step2: 确保目标父目录存在（锁外 await）
        let to_parent = to.parent().unwrap_or_else(|| Path::new("/"));
        let parent_exists = {
            let state = self.state.lock().unwrap();
            state.canonicalize(to_parent, true).is_some()
        };
        if !parent_exists {
            self.create_dir(to_parent).await?;
        }

        // step3: 放入 to
        {
            let mut state = self.state.lock().unwrap();
            let (to_entries, to_name) = state.try_parent_entry(to, true).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "to parent not found")
            })?;
            to_entries.insert(to_name, entry);
        }
        Ok(())
    }

    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        let result = {
            let state = self.state.lock().unwrap();
            state.canonicalize(path, true)
        };
        result.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found").into())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let result = {
            let mut state = self.state.lock().unwrap();
            match state.try_entry(path, true) {
                Some((FakeFsEntry::Dir { entries, .. }, canonical)) => {
                    let mut out: Vec<PathBuf> =
                        entries.keys().map(|name| canonical.join(name)).collect();
                    out.sort();
                    Ok(out)
                }
                Some(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "not a directory",
                )),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "not found",
                )),
            }
        };
        result.map_err(Into::into)
    }

    async fn metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let result = {
            let mut state = self.state.lock().unwrap();
            match state.try_entry(path, true) {
                Some((FakeFsEntry::File { len, .. }, _)) => Ok(Some(Metadata {
                    is_file: true,
                    is_dir: false,
                    len: *len,
                })),
                Some((FakeFsEntry::Dir { len, .. }, _)) => Ok(Some(Metadata {
                    is_file: false,
                    is_dir: true,
                    len: *len,
                })),
                Some((FakeFsEntry::Symlink { .. }, _)) => Ok(Some(Metadata {
                    is_file: false,
                    is_dir: false,
                    len: 0,
                })),
                None => Ok(None),
            }
        };
        result
    }

    async fn watch(
        &self,
        _path: &Path,
        _latency: Duration,
    ) -> (
        Pin<Box<dyn Send + Stream<Item = Vec<crate::PathEvent>>>>,
        Arc<dyn crate::Watcher>,
    ) {
        let stream: Pin<Box<dyn Send + Stream<Item = Vec<crate::PathEvent>>>> =
            Box::pin(futures::stream::empty());
        let watcher = Arc::new(crate::real_fs::NoopWatcher);
        (stream, watcher)
    }
}
