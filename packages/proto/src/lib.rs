#![allow(non_snake_case)]

pub mod error;
mod macros;
mod typed_envelope;

#[cfg(test)]
mod tests;

pub use error::*;
pub use prost::{DecodeError, Message};
pub use typed_envelope::*;

// 协议类型由 build script 生成的 `zed.messages.rs` 提供。
// 这一段必须留在 crate 根：所有宏调用都依赖它生成的类型名。
include!(concat!(env!("OUT_DIR"), "/zed.messages.rs"));

// 宏调用列表 + impl 块。`include!` 是文本内联，展开后仍在 crate 根，
// 因此宏内部对生成类型的引用方式不变。
include!("messages.rs");
include!("request_messages.rs");
include!("lsp_messages.rs");
include!("entity_messages.rs");

// 独立模块：常量、非序列化拆分工具等。
pub mod constants;
pub mod nonce;
pub mod split;
pub mod timestamp;

pub use constants::{REMOTE_SERVER_PEER_ID, REMOTE_SERVER_PROJECT_ID};
pub use split::{
    MAX_WORKTREE_UPDATE_MAX_CHUNK_SIZE, split_repository_update, split_worktree_update,
};
