use std::time::Duration;

pub const FS_WATCH_LATENCY: Duration = Duration::from_millis(100);

/// How often the background scanner verifies that the worktree root still
/// exists at its recorded path.
pub const ROOT_PATH_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub(crate) const STREAM_BLOCK_BYTES: usize = 1024 * 1024;

#[cfg(feature = "test-support")]
pub(crate) const SENTINEL_RETRY_TICKS: usize = 10;
