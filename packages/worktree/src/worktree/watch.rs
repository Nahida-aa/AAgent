use super::*;

pub(crate) struct NullWatcher;
impl fs::Watcher for NullWatcher {
    fn add(&self, _path: &Path) -> Result<()> { Ok(()) }

    fn remove(&self, _path: &Path) -> Result<()> { Ok(()) }
}
