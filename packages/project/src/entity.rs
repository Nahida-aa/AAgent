//! Project 核心类型。
//!
//! 一个 Project = 一个被 AAgent 打开的目录树（对应 Zed 的 `Project`）。
//! 负责：
//! - 持有项目根路径 + 所有 worktree
//! - 管理 per-project settings（`.aa/settings.json`）
//! - 持有子系统 store（git、lsp、buffers、terminals ...）
//! - 通过 `fs: Arc<dyn Fs>` 做所有文件操作（可测试）
//!
//! 注意：当前版本是**纯数据骨架**，不依赖 GPUI。
//! 升级为 GPUI Entity 时（像 Zed 那样）再把 Entity 字段加进来。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::fs::{Fs, RealFs};

/// Worktree 标识（等价于 Zed 的 WorktreeId）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorktreeId(pub u64);

/// Project 打开时的初始 worktree 策略。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenWorktreeStrategy {
    /// 打开根目录，worktrees 留空。
    #[default]
    None,
    /// 自动发现所有 git worktrees。
    GitWorktrees,
}

/// Project 初始化参数。
pub struct OpenProjectOptions {
    pub worktree_strategy: OpenWorktreeStrategy,
    /// 文件系统实现——生产用 RealFs::new()，测试用 MockFs::new_arc()。
    /// None 时默认 RealFs。
    pub fs: Option<Arc<dyn Fs>>,
}

impl Clone for OpenProjectOptions {
    fn clone(&self) -> Self {
        Self {
            worktree_strategy: self.worktree_strategy.clone(),
            fs: self.fs.clone(),
        }
    }
}

impl std::fmt::Debug for OpenProjectOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenProjectOptions")
            .field("worktree_strategy", &self.worktree_strategy)
            .field("fs", &self.fs.as_ref().map(|_| "<dyn Fs>"))
            .finish()
    }
}

impl Default for OpenProjectOptions {
    fn default() -> Self {
        Self {
            worktree_strategy: OpenWorktreeStrategy::None,
            fs: None,
        }
    }
}

impl OpenProjectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fs(mut self, fs: Arc<dyn Fs>) -> Self {
        self.fs = Some(fs);
        self
    }
}

/// Semantics-aware entity that is relevant to one or more worktrees with the files.
///
/// `Project` 是整个 IDE 的"根数据容器"——持有 worktrees、settings、
/// 以及所有子系统（git、lsp、buffers、terminals ...）的 store。
///
/// 对齐 Zed `crates/project/src/project.rs::Project`。
/// 当前版本为纯数据层（无 GPUI 依赖），runtime 方法做 stub。
pub struct Project {
    /// 文件系统实现——所有文件操作（读 settings、git worktree 扫描）都走它。
    /// Zed 对应 `fs: Arc<dyn Fs>`。
    fs: Arc<dyn Fs>,

    /// 项目根路径（worktree_id = 0 的 worktree 路径）。
    root_path: PathBuf,

    /// 所有 worktree：主线（id=0）+ git worktrees。
    ///
    /// Zed 用 `Entity<WorktreeStore>` 管理；我们当前用纯 `Vec<WorktreeEntry>`。
    worktrees: Vec<WorktreeEntry>,

    /// Per-project settings 路径（根路径/.aa/settings.json）。
    settings_path: PathBuf,
}

/// 一个 worktree 的基本信息。
/// Zed 里 Worktree 是独立 Entity，当前我们只存元数据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub id: WorktreeId,
    pub path: PathBuf,
    pub is_main: bool,
}

impl Project {
    /// 打开一个项目——用根路径初始化，不加载 worktrees。
    /// 完整逻辑（git worktree 发现、LSP 启动、Git store 初始化）在 runtime 层。
    pub fn open(root_path: impl Into<PathBuf>, options: OpenProjectOptions) -> Self {
        let root_path = root_path.into();
        let fs = options.fs.clone().unwrap_or_else(RealFs::new);

        let mut worktrees = vec![WorktreeEntry {
            id: WorktreeId(0),
            path: root_path.clone(),
            is_main: true,
        }];

        // 扫描 git worktrees（同步、简短）。
        // Zed 异步做这个；我们先同步 stub。
        if let Some(git_wt) = Self::discover_git_worktrees(&fs, &root_path) {
            worktrees.extend(git_wt);
        }

        Self {
            fs,
            root_path: root_path.clone(),
            worktrees,
            settings_path: root_path.join(".aa").join("settings.json"),
        }
    }

    /// 文件系统实现。
    pub fn fs(&self) -> &Arc<dyn Fs> {
        &self.fs
    }

    /// 项目根路径。
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// 所有 worktree（含主线）。
    pub fn worktrees(&self) -> &[WorktreeEntry] {
        &self.worktrees
    }

    /// 主线 worktree（id=0）。
    pub fn main_worktree(&self) -> &WorktreeEntry {
        // 构造时保证第一个就是主线
        &self.worktrees[0]
    }

    /// Per-project settings 文件路径。
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    /// 找到某个路径所属的 worktree。
    pub fn worktree_for_path(&self, path: &Path) -> Option<&WorktreeEntry> {
        // 匹配最长前缀
        self.worktrees
            .iter()
            .filter(|wt| path.starts_with(&wt.path))
            .max_by_key(|wt| wt.path.components().count())
    }

    /// 从某个相对路径构造 ProjectPath（worktree_id + 相对路径）。
    pub fn project_path(&self, path: &Path) -> Option<ProjectPath> {
        let wt = self.worktree_for_path(path)?;
        let relative = path.strip_prefix(&wt.path).ok()?;
        Some(ProjectPath {
            worktree_id: wt.id,
            path: relative.to_path_buf(),
        })
    }

    // ---------- git worktree discovery ----------

    fn discover_git_worktrees(fs: &Arc<dyn Fs>, root: &Path) -> Option<Vec<WorktreeEntry>> {
        let git_dir = root.join(".git");
        if !fs.path_exists(&git_dir) {
            return None;
        }
        // 调 `git worktree list --porcelain`
        let output = std::process::Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(root)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut id = 1_u64;
        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                let path = PathBuf::from(path.trim());
                if path == root {
                    continue; // skip main worktree（id=0）
                }
                worktrees.push(WorktreeEntry {
                    id: WorktreeId(id),
                    path,
                    is_main: false,
                });
                id += 1;
            }
        }
        Some(worktrees)
    }
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("root_path", &self.root_path)
            .field("worktrees", &self.worktrees)
            .field("settings_path", &self.settings_path)
            .finish()
    }
}

// ---------- ProjectPath ----------

/// 一个 worktree 内的相对路径。
/// Zed 里还有 `ProjectEntryId`（worktree 内目录索引），我们暂时不用。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectPath {
    pub worktree_id: WorktreeId,
    pub path: PathBuf,
}

impl ProjectPath {
    pub fn new(worktree_id: WorktreeId, path: impl Into<PathBuf>) -> Self {
        Self {
            worktree_id,
            path: path.into(),
        }
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_non_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let project = Project::open(tmp.path(), OpenProjectOptions::default());
        assert_eq!(project.worktrees().len(), 1);
        assert!(project.main_worktree().is_main);
        assert_eq!(project.main_worktree().path, tmp.path());
        assert_eq!(
            project.settings_path(),
            tmp.path().join(".aa").join("settings.json")
        );
    }

    #[test]
    fn test_worktree_for_path() {
        let tmp = tempfile::tempdir().unwrap();
        let project = Project::open(tmp.path(), OpenProjectOptions::default());
        let file = tmp.path().join("src").join("main.rs");
        let wt = project.worktree_for_path(&file).unwrap();
        assert!(wt.is_main);
    }

    #[test]
    fn test_project_path() {
        let tmp = tempfile::tempdir().unwrap();
        let project = Project::open(tmp.path(), OpenProjectOptions::default());
        let file = tmp.path().join("src").join("main.rs");
        let pp = project.project_path(&file).unwrap();
        assert_eq!(pp.worktree_id, WorktreeId(0));
        assert_eq!(pp.path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_git_worktree_discovery() {
        let tmp = tempfile::tempdir().unwrap();
        // init repo + main branch
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.test"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::fs::write(tmp.path().join("file.txt"), "hello").unwrap();
        std::process::Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-qm", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        // 创建 worktree
        let wt_dir = tmp.path().parent().unwrap().join("wt_test_branch");
        std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "-q",
                wt_dir.to_str().unwrap(),
                "-b",
                "feature-test",
            ])
            .current_dir(tmp.path())
            .status()
            .unwrap();

        let project = Project::open(tmp.path(), OpenProjectOptions::default());
        assert!(
            project.worktrees().len() >= 2,
            "expected main + worktree, got {:?}",
            project.worktrees()
        );

        // 清理
        let _ = std::fs::remove_dir_all(&wt_dir);
        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(tmp.path())
            .status();
    }
}
