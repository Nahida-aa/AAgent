// ├── opening/
// │   ├── mod.rs
// │   ├── options.rs             # OpenOptions / OpenResult / OpenMode
// │   ├── matching.rs            # WorkspaceMatching / find_existing_workspace
// │   ├── local.rs               # open_paths / open_workspace_by_id / open_new
// │   ├── remote.rs              # open_remote_project_*
// │   ├── prompts.rs             # prompt_and_open_paths
// │   ├── restore.rs             # restore_multiworkspace
// │   └── windows.rs             # workspace_windows_for_location

// OpenOptions / OpenResult / OpenMode, OpenVisible
use super::*;
mod options;
// prompt_and_open_paths, prompt_for_open_path_and_open, PromptForNewPath, PromptForOpenPath
mod prompt;
pub mod file;
pub mod local;
pub mod matching;
pub mod ops;
pub mod remote;
pub mod restore;
pub mod windows;
pub use options::{OpenMode, OpenOptions, OpenVisible};
pub use prompt::{PromptForNewPath, PromptForOpenPath};
