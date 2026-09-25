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

mod actions;
mod event;
mod debounce;
mod keystrokes;
mod construct;
