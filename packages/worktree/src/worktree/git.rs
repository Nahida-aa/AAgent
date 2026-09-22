use super::*;

pub(crate) async fn discover_ancestor_git_repo(
    fs: Arc<dyn Fs>,
    root_abs_path: &SanitizedPath,
) -> (
    HashMap<Arc<Path>, (Arc<Gitignore>, bool)>,
    Option<Arc<Gitignore>>,
    Option<(PathBuf, WorkDirectory)>,
) {
    let mut exclude = None;
    let mut ignores = HashMap::default();
    for (index, ancestor) in root_abs_path.as_path().ancestors().enumerate() {
        if index != 0 {
            if ancestor == paths::home_dir() {
                // Unless $HOME is itself the worktree root, don't consider it as a
                // containing git repository---expensive and likely unwanted.
                break;
            } else if let Ok(ignore) = build_gitignore(&ancestor.join(GITIGNORE), fs.as_ref()).await
            {
                ignores.insert(ancestor.into(), (ignore.into(), false));
            }
        }

        let ancestor_dot_git = ancestor.join(DOT_GIT);
        log::trace!("considering ancestor: {ancestor_dot_git:?}");
        // Check whether the directory or file called `.git` exists (in the
        // case of worktrees it's a file.)
        if fs
            .metadata(&ancestor_dot_git)
            .await
            .is_ok_and(|metadata| metadata.is_some())
        {
            let dot_git_abs_path = if index != 0 {
                // We canonicalize, since the FS events use the canonicalized path.
                match fs.canonicalize(&ancestor_dot_git).await.log_err() {
                    Some(path) => path,
                    None => continue,
                }
            } else {
                ancestor_dot_git.clone()
            };
            let dot_git_abs_path: Arc<Path> = dot_git_abs_path.as_path().into();
            let (_, common_dir_abs_path) = discover_git_paths(&dot_git_abs_path, fs.as_ref()).await;

            let repo_exclude_abs_path = common_dir_abs_path.join(REPO_EXCLUDE);
            if let Ok(repo_exclude) =
                build_gitignore_with_root(&repo_exclude_abs_path, ancestor, fs.as_ref()).await
            {
                exclude = Some(Arc::new(repo_exclude));
            }

            if index != 0 {
                let location_in_repo = root_abs_path
                    .as_path()
                    .strip_prefix(ancestor)
                    .unwrap()
                    .into();
                log::info!("inserting parent git repo for this worktree: {location_in_repo:?}");
                // We associate the external git repo with our root folder and
                // also mark where in the git repo the root folder is located.
                return (
                    ignores,
                    exclude,
                    Some((
                        dot_git_abs_path.as_ref().into(),
                        WorkDirectory::AboveProject {
                            absolute_path: ancestor.into(),
                            location_in_repo,
                        },
                    )),
                );
            }

            break;
        }
    }

    (ignores, exclude, None)
}
pub async fn discover_root_repo_common_dir(root_abs_path: &Path, fs: &dyn Fs) -> Option<Arc<Path>> {
    discover_root_repo_metadata(root_abs_path, fs)
        .await
        .map(|(common_dir, _)| common_dir)
}

pub(crate) async fn discover_root_repo_metadata(
    root_abs_path: &Path,
    fs: &dyn Fs,
) -> Option<(Arc<Path>, bool)> {
    let root_dot_git = root_abs_path.join(DOT_GIT);
    if !fs.metadata(&root_dot_git).await.is_ok_and(|m| m.is_some()) {
        return None;
    }
    let dot_git_path: Arc<Path> = root_dot_git.into();
    let (repository_dir, common_dir) = discover_git_paths(&dot_git_path, fs).await;
    let is_linked_worktree = repository_dir != common_dir;
    Some((common_dir, is_linked_worktree))
}

pub(crate) async fn discover_git_paths(
    dot_git_abs_path: &Arc<Path>,
    fs: &dyn Fs,
) -> (Arc<Path>, Arc<Path>) {
    let mut repository_dir_abs_path = dot_git_abs_path.clone();
    let mut common_dir_abs_path = dot_git_abs_path.clone();

    if let Some(path) = fs
        .load(dot_git_abs_path)
        .await
        .ok()
        .as_ref()
        .and_then(|contents| parse_gitfile(contents).log_err())
    {
        let path = resolve_gitfile_path(dot_git_abs_path, path);
        if let Some(path) = fs.canonicalize(&path).await.log_err() {
            repository_dir_abs_path = Path::new(&path).into();
            common_dir_abs_path = repository_dir_abs_path.clone();

            if let Some(commondir_contents) = fs.load(&path.join("commondir")).await.ok()
                && let Some(commondir_path) = fs
                    .canonicalize(&resolve_commondir_path(&path, &commondir_contents))
                    .await
                    .log_err()
            {
                common_dir_abs_path = commondir_path.as_path().into();
            }
        }
    };
    (repository_dir_abs_path, common_dir_abs_path)
}

/// Watches the directories inside a git directory that git writes ref updates to.
///
/// On Linux and FreeBSD the native file watcher is non-recursive, so a watch on the git
/// directory itself does not report changes to files nested below it, such as the loose
/// refs that git updates on commit, fetch, and branch operations. Watch the `refs` tree
/// (its directories are watched individually because branch names may contain slashes)
/// and, for repositories using the reftable backend, the `reftable` directory. On
/// platforms with recursive watchers these calls are deduplicated against the existing
/// recursive registration, making them effectively free.
pub(crate) async fn watch_git_dir_subdirectories(git_dir_abs_path: &Path, fs: &dyn Fs, watcher: &dyn Watcher) {
    let reftable_dir_abs_path = git_dir_abs_path.join(REFTABLE_DIR);
    if fs.is_dir(&reftable_dir_abs_path).await {
        watcher
            .add(&reftable_dir_abs_path)
            .context("failed to add reftable directory to watcher")
            .log_err();
    }

    watch_dir_tree(git_dir_abs_path.join(REFS_DIR), fs, watcher).await;
}

/// Watches a directory and all of its descendant directories.
///
/// Each directory is watched before its children are enumerated, so that a child
/// created concurrently is either seen by the enumeration or reported by the watch.
pub(crate) async fn watch_dir_tree(root_abs_path: PathBuf, fs: &dyn Fs, watcher: &dyn Watcher) {
    let mut dirs_to_watch = vec![root_abs_path];
    while let Some(dir_abs_path) = dirs_to_watch.pop() {
        if !fs.is_dir(&dir_abs_path).await {
            continue;
        }
        watcher
            .add(&dir_abs_path)
            .with_context(|| format!("failed to watch directory {dir_abs_path:?}"))
            .log_err();
        let Some(mut children) = fs.read_dir(&dir_abs_path).await.log_err() else {
            continue;
        };
        while let Some(child_abs_path) = children.next().await {
            let Some(child_abs_path) = child_abs_path.log_err() else {
                continue;
            };
            if fs.is_dir(&child_abs_path).await {
                dirs_to_watch.push(child_abs_path);
            }
        }
    }
}
pub(crate) async fn is_dot_git(path: &Path, fs: &dyn Fs) -> bool {
    if let Some(file_name) = path.file_name()
        && file_name == DOT_GIT
    {
        return true;
    }

    // If we're in a bare repository, we are not inside a `.git` folder. In a
    // bare repository, the root folder contains what would normally be in the
    // `.git` folder.
    let head_metadata = fs.metadata(&path.join("HEAD")).await;
    if !matches!(head_metadata, Ok(Some(_))) {
        return false;
    }
    let config_metadata = fs.metadata(&path.join("config")).await;
    matches!(config_metadata, Ok(Some(_)))
}

fn parse_gitfile(content: &str) -> anyhow::Result<&Path> {
    let path = content
        .strip_prefix("gitdir:")
        .with_context(|| format!("parsing gitfile content {content:?}"))?;
    Ok(Path::new(path.trim()))
}

fn resolve_gitfile_path(dot_git_abs_path: &Path, gitfile_path: &Path) -> PathBuf {
    if gitfile_path.is_absolute() {
        gitfile_path.into()
    } else {
        dot_git_abs_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(gitfile_path)
    }
}

fn resolve_commondir_path(repository_dir_abs_path: &Path, commondir_path: &str) -> PathBuf {
    let commondir_path = Path::new(commondir_path.trim());
    if commondir_path.is_absolute() {
        commondir_path.into()
    } else {
        repository_dir_abs_path.join(commondir_path)
    }
}
