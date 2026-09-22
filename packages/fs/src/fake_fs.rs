use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use async_tar::Archive;
use collections::{BTreeMap, btree_map};
use futures::StreamExt;
use futures::{AsyncRead, Stream};
use gpui::BackgroundExecutor;
use parking_lot::Mutex;
use slotmap::SlotMap;

use git::{
    repository::{CommitData, InitialGraphCommitData, RepoPath, Worktree, repo_path},
    status::{FileStatus, StatusCode, TrackedStatus, UnmergedStatus},
};
use path::normalize_path;
use smol::io::AsyncReadExt;
use std::ffi::OsStr;

use crate::fake_git_repo::{FakeCommitDataEntry, FakeGitRepository, FakeGitRepositoryState};
use crate::file_handle::FileHandle;
use crate::fs_watcher;
use crate::jobs::{JobEventReceiver, JobEventSender};
use crate::metadata::{MTime, Metadata};
use crate::options::{CopyOptions, CreateOptions, RemoveOptions, RenameOptions};
use crate::traits::Fs;
use crate::trash::{TrashId, TrashRestoreError, TrashedEntry};
use crate::watcher::{PathEvent, PathEventKind, Watcher};
use crate::{PathBuf as _, read_dir_items}; // 只是示意；实际按需导入

// ...（FakeFs / FakeFsEntry / FakeWatches / FakeWatchBackend / FakeHandle
//     以及所有 `impl Fs for FakeFs` 的代码，从原文件按原样搬迁即可）
