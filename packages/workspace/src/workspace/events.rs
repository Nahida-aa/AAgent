use crate::persistence::model::SerializedWorkspaceLocation;

pub enum Event {
    PaneAdded(Entity<Pane>),
    PaneRemoved,
    ItemAdded {
        item: Box<dyn ItemHandle>,
    },
    ActiveItemChanged,
    ItemRemoved {
        item_id: EntityId,
    },
    UserSavedItem {
        pane: WeakEntity<Pane>,
        item: Box<dyn WeakItemHandle>,
        save_intent: SaveIntent,
    },
    ContactRequestedJoin(u64),
    WorkspaceCreated(WeakEntity<Workspace>),
    OpenBundledFile {
        text: Cow<'static, str>,
        title: &'static str,
        language: &'static str,
    },
    ZoomChanged,
    ModalOpened,
    Activate,
    PanelAdded(AnyView),
    WorktreeCreationChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoWatch {
    Off,
    Active { watched_peer: Option<PeerId> },
    Paused,
}

impl AutoWatch {
    pub fn enabled(&self) -> bool { matches!(self, AutoWatch::Active { .. } | AutoWatch::Paused) }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseIntent {
    /// Quit the program entirely.
    Quit,
    /// Close a window.
    CloseWindow,
    /// Replace the workspace in an existing window.
    ReplaceWindow,
}

/// Controls which types of items should be made visible in the project panel
/// when opened.
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

enum WorkspaceLocation {
    // Valid local paths or SSH project to serialize
    Location(SerializedWorkspaceLocation, PathList),
    // No valid location found to serialize
    None,
}
