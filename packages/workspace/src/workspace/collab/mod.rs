use super::Workspace;
use super::*;
use crate::dock::Dock;
use crate::workspace::core::WorkspaceId;
use crate::workspace::core::lifecycle::CloseIntent;
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use gpui::{App, AsyncApp, Context, Entity, PromptLevel, Task, WeakEntity, Window};

use crate::pane::Pane;

// collab：通话、频道、共享项目
// 是什么
// 通话/房间（call/room）：你加入一个频道（channel）后进入的通话。

// 共享项目（shared project）：把当前 project 共享给房间里的其他人。

// 参与者（participant）：房间里的其他人，每人有 PeerId 和 ParticipantIndex。

// 屏幕共享（screen share）：把某人的屏幕作为 item 打开。
// 干嘛的:
// - 维护“我现在在不在通话里”“在哪个频道”“共享了哪个项目”
// - 提供房间内参与者信息
// - 处理加入/离开频道、共享/取消共享项目
// - 提供 SharedScreen 作为可打开的 item
//
// workspace/collab/
// ├── mod.rs
// ├── call.rs               # AnyActiveCall / GlobalAnyActiveCall
// ├── event.rs               # ActiveCallEvent / on_active_call_event
// ├── participant.rs        # RemoteCollaborator / ParticipantLocation
// ├── channel.rs            # join_channel / join_channel_internal
// └── room_project.rs       # join_in_room_project

pub mod actions;
pub mod call;
pub mod channel;
pub mod event;
pub mod participant;
pub mod read;
pub mod room_project;
pub use call::{AnyActiveCall, GlobalAnyActiveCall};
