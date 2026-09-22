use super::*;

pub(crate) async fn build_gitignore(abs_path: &Path, fs: &dyn Fs) -> Result<Gitignore> {
    let parent = abs_path.parent().unwrap_or_else(|| Path::new("/"));
    build_gitignore_with_root(abs_path, parent, fs).await
}

pub(crate) async fn build_gitignore_with_root(
    abs_path: &Path,
    root: &Path,
    fs: &dyn Fs,
) -> Result<Gitignore> {
    let contents = fs
        .load(abs_path)
        .await
        .with_context(|| format!("failed to load gitignore file at {}", abs_path.display()))?;
    let mut builder = GitignoreBuilder::new(root);
    for line in contents.lines() {
        builder.add_line(Some(abs_path.into()), line)?;
    }
    Ok(builder.build()?)
}
