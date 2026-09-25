use super::Workspace;
use crate::{dock::Dock, workspace::event::CloseIntent};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Context, Entity, PromptLevel, Task, Window};

// follow：视图跟随
// 是什么
// Leader（被跟随者）：你正在看谁的编辑器。

// Follower（跟随者）：正在看你的人。

// View（视图）：一个被跟随者打开的 item 的可序列化状态，用 ViewId 标识。

// AutoWatch：自动跟随第一个开始共享屏幕的人。

// 干嘛的
// 你按“Follow”后，你的 pane 变成 leader 的镜像：

// leader 切文件，你的 pane 跟着切。

// leader 在某个位置，你的光标/位置跟着动。

// leader 通过 proto::UpdateFollowers 广播视图变更。

// follower 用 FollowableViewRegistry::from_state_proto 重建 item。

// 用于协作演示、pair programming、看别人操作。
//
//
// workspace/follow/
// ├── mod.rs
// ├── state.rs              # FollowerState / Follower / FollowerView / ViewId / CollaboratorId
// ├── leader.rs             # update_active_view_for_followers / update_followers / handle_follow
// ├── follower.rs           # process_leader_update / add_view_from_leader / leader_updated
// ├── agent.rs              # handle_agent_location_changed / active_item_for_agent
// ├── auto_watch.rs         # AutoWatch / toggle_auto_watch
// ├── shared_screen.rs      # open_shared_screen / shared_screen_for_peer
// ├── rpc.rs                 # WorkspaceStore 里的 follow RPC
// └── render.rs             # leader_border_for_pane
//
// collab（通话/频道/共享项目）
//    │
//    │ 提供：PeerId、remote_participant_for_peer_id、room_id、
//    │       peer_ids_with_video_tracks、create_shared_screen
//    ▼
// follow（视图跟随 / 自动跟随 / 屏幕共享）
//    │
//    │ 依赖：collab 才能知道“follow 谁”、“谁在共享屏幕”
//    ▼
// FollowerState + FollowableItemHandle + ViewId
//
// 没有通话，就没有 follow：start_following 里要 self.active_call()?.room_id(cx)?。
//
// follow 需要 collab 的参与者信息：active_item_for_peer 里要 call.remote_participant_for_peer_id(peer_id, cx)。

// AutoWatch 需要 collab 的视频轨信息：next_watched_peer 里要 call.peer_ids_with_video_tracks(cx)。

// 屏幕共享是 follow 与 collab 的交界：AnyActiveCall::create_shared_screen 由 collab 提供，open_shared_screen 由 follow 使用。
//
// workspace/follow/state.rs
// FollowerState / Follower / FollowerView / ViewId / CollaboratorId
mod state;
