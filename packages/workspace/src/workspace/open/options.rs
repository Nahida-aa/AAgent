/// Controls which types of items should be made visible in the project panel
/// when opened.
use super::*;
#[derive(Debug, Clone)]
pub enum OpenVisible {
    /// Make all opened items visible (both files and directories).
    All,
    /// Don't make any opened items visible.
    None,
    /// Only make opened files visible, not directories.
    OnlyFiles,
    /// Only make opened directories visible, not files.
    OnlyDirectories,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpenMode {
    /// Open the workspace in a new window.
    NewWindow,
    /// Add to the window's multi workspace without activating it (used during deserialization).
    Add,
    /// Add to the window's multi workspace and activate it.
    #[default]
    Activate,
}

#[derive(Clone)]
pub struct OpenOptions {
    pub visible: Option<OpenVisible>,
    pub focus: Option<bool>,
    pub workspace_matching: WorkspaceMatching,
    /// Whether to add unmatched directories to the existing window's sidebar
    /// rather than opening a new window. Defaults to true, matching the default
    /// `cli_default_open_behavior` setting.
    pub add_dirs_to_sidebar: bool,
    pub wait: bool,
    pub requesting_window: Option<WindowHandle<MultiWorkspace>>,
    pub open_mode: OpenMode,
    pub env: Option<HashMap<String, String>>,
    pub open_in_dev_container: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            visible: None,
            focus: None,
            workspace_matching: WorkspaceMatching::default(),
            add_dirs_to_sidebar: true,
            wait: false,
            requesting_window: None,
            open_mode: OpenMode::default(),
            env: None,
            open_in_dev_container: false,
        }
    }
}

impl OpenOptions {
   pub(crate) fn should_reuse_existing_window(&self) -> bool {
        !matches!(
            self.workspace_matching,
            WorkspaceMatching::None | WorkspaceMatching::MatchSubpaths
        ) && self.open_mode != OpenMode::NewWindow
    }
}

/// The result of opening a workspace via [`open_paths`], [`Workspace::new_local`],
/// or [`Workspace::open_workspace_for_paths`].
pub struct OpenResult {
    pub window: WindowHandle<MultiWorkspace>,
    pub workspace: Entity<Workspace>,
    pub opened_items: Vec<Option<anyhow::Result<Box<dyn ItemHandle>>>>,
}

#[derive(Debug)]
pub struct WorkspacePosition {
    pub window_bounds: Option<WindowBounds>,
    pub display: Option<Uuid>,
    pub centered_layout: bool,
}
