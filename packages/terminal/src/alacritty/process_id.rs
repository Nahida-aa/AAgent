#[cfg(target_os = "windows")]
use std::num::NonZeroU32;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(target_os = "windows")]
use windows::Win32::{Foundation::HANDLE, System::Threading::GetProcessId};

use crate::pty_info::ProcessIdGetter;

use super::AlacrittyPty;

#[cfg(unix)]
impl From<&AlacrittyPty> for ProcessIdGetter {
    fn from(pty: &AlacrittyPty) -> Self { Self::new(pty.file().as_raw_fd(), pty.child().id()) }
}

#[cfg(windows)]
impl From<&AlacrittyPty> for ProcessIdGetter {
    fn from(pty: &AlacrittyPty) -> Self {
        let child = pty.child_watcher();
        let handle = child.raw_handle();
        let fallback_pid = child.pid().unwrap_or_else(|| unsafe {
            NonZeroU32::new_unchecked(GetProcessId(HANDLE(handle as _)))
        });

        Self::new(handle as i32, u32::from(fallback_pid))
    }
}
