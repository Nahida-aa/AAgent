use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Bounds, WindowBounds};
use uuid::Uuid;



/// Controls which types of items should be made visible in the project panel
/// when opened.
#[derive(Debug, Clone)]
pub enum OpenVisible {
    All,
    None,
    OnlyFiles,
    OnlyDirectories,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpenMode {
    NewWindow,
    Add,
    #[default]
    Activate,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WorkspaceMatching {
    None,
    #[default]
    MatchExact,
    MatchSubpaths,
    MatchSubdirectory,
}

#[derive(Clone)]
pub struct OpenOptions {
    pub visible: Option<OpenVisible>,
    pub focus: Option<bool>,
    pub workspace_matching: WorkspaceMatching,
    pub add_dirs_to_sidebar: bool,
    pub wait: bool,
    pub requesting_window: Option<gpui::WindowHandle<crate::MultiWorkspace>>,
    pub open_mode: OpenMode,
    pub env: Option<collections::HashMap<String, String>>,
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
    fn should_reuse_existing_window(&self) -> bool {
        !matches!(
            self.workspace_matching,
            WorkspaceMatching::None | WorkspaceMatching::MatchSubpaths
        ) && self.open_mode != OpenMode::NewWindow
    }
}

pub struct OpenResult {
    pub window: gpui::WindowHandle<crate::MultiWorkspace>,
    pub workspace: gpui::Entity<crate::Workspace>,
    pub opened_items: Vec<Option<anyhow::Result<Box<dyn crate::ItemHandle>>>>,
}

pub struct WorkspacePosition {
    pub window_bounds: Option<WindowBounds>,
    pub display: Option<Uuid>,
    pub centered_layout: bool,
}

pub(crate) enum WorkspaceLocation {
    Location(
        crate::persistence::model::SerializedWorkspaceLocation,
        crate::PathList,
    ),
    None,
}
