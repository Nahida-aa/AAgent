// workspace/core/
// ├── mod.rs
// ├── workspace.rs          # Workspace 结构体定义
// ├── ids.rs                # WorkspaceId
// ├── event.rs             # Event + impl EventEmitter<Event>
// ├── debounce.rs           # DelayedDebouncedEditAction
// ├── keystrokes.rs         # DispatchingKeystrokes
// ├── construct.rs          # new / new_local / test_new
// ├── lifecycle.rs          # prepare_to_close / reload / flush / CloseIntent
// ├── read.rs          # weak_handle / app_state / project / ...
// └── actions.rs            # namespace = workspace 的 action

pub mod actions;
pub mod construct;
pub mod debounce;
pub mod event;
mod ids;
pub use ids::WorkspaceId;
pub mod keystrokes;
pub mod workspace;
// weak_handle
// app_state
// user_store
// project
// path_style
// project_group_key          // 转发 project.project_group_key
// multi_workspace
// is_edited                  // 也可放 window/chrome.rs，见下
// is_restoring
// status_bar
// status_bar_visible
// set_sidebar_focus_handle   // setter，但只是字段写，可放这里
// set_panels_task
// take_panels_task
// set_multi_workspace        // 写字段 + 更新 status_bar
mod read;
