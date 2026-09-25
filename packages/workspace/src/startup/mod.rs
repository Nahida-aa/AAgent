mod local;
mod prompt;
mod remote;
mod restore;
mod window_utils;

pub use local::{
    create_and_open_local_file, find_existing_workspace, open_new, open_paths, open_workspace_by_id,
};
pub use prompt::{init, prompt_and_open_paths, prompt_for_open_path_and_open};
pub use remote::{
    deserialize_remote_project, open_remote_project_inner,
    open_remote_project_with_existing_connection, open_remote_project_with_new_connection,
};
pub use restore::{
    apply_restored_multiworkspace_state, last_opened_workspace_location,
    last_session_workspace_locations, restore_multiworkspace, restore_native_window_state,
};
pub use window_utils::{
    activate_any_workspace_window, get_any_active_multi_workspace, workspace_windows_for_location,
};
