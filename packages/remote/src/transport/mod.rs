pub mod docker;
#[cfg(any(test, feature = "test-support"))]
pub mod mock;
pub mod ssh;
pub mod wsl;

mod build_server;
mod platform;
mod shell;
mod stdio;

#[cfg(test)]
mod tests;

// 这些函数原本是 transport 模块的私有项，被 ssh / wsl / docker 等
// 子模块通过 `super::xxx` 引用。为了让这些子模块的调用点不变，
// 在 mod.rs 里 `pub(crate) use` 一下即可。
pub(crate) use build_server::build_remote_server_from_source;
pub(crate) use platform::{
    os_version_command, parse_os_version, parse_platform, parse_windows_version,
};
pub(crate) use shell::parse_shell;
pub(crate) use stdio::handle_rpc_messages_over_child_process_stdio;
