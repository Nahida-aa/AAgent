/// Opens a file or directory.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct Open {
    /// When true, opens in a new window. When false, adds to the current
    /// window as a new workspace (multi-workspace). When omitted, uses
    /// `default_open_behavior`.
    #[serde(default)]
    pub create_new_window: Option<bool>,
}
impl Open {
    pub const DEFAULT: Self = Self {
        create_new_window: None,
    };
}

impl Default for Open {
    fn default() -> Self { Self::DEFAULT }
}

/// Activates a specific pane by its index.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct ActivatePane(pub usize);

/// Moves an item to a specific pane by index.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct MoveItemToPane {
    #[serde(default = "default_1")]
    pub destination: usize,
    #[serde(default = "default_true")]
    pub focus: bool,
    #[serde(default)]
    pub clone: bool,
}

fn default_1() -> usize { 1 }
/// Moves an item to a pane in the specified direction.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct MoveItemToPaneInDirection {
    #[serde(default = "default_right")]
    pub direction: SplitDirection,
    #[serde(default = "default_true")]
    pub focus: bool,
    #[serde(default)]
    pub clone: bool,
}

/// Creates a new file in a split of the desired direction.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewFileSplit(pub SplitDirection);
fn default_right() -> SplitDirection { SplitDirection::Right }

/// Saves all open files in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct SaveAll {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Saves the current file with the specified options.
#[derive(Clone, PartialEq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct Save {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Moves Focus to the central panes in the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = workspace)]
pub struct FocusCenterPane;

///  Closes all items and panes in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseAllItemsAndPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Closes all inactive tabs and panes in the workspace.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseInactiveTabsAndPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
}

/// Closes the active item across all panes.
#[derive(Clone, PartialEq, Debug, Deserialize, Default, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct CloseItemInAllPanes {
    #[serde(default)]
    pub save_intent: Option<SaveIntent>,
    #[serde(default)]
    pub close_pinned: bool,
}

/// Sends a sequence of keystrokes to the active element.
#[derive(Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
pub struct SendKeystrokes(pub String);

actions!(
    workspace,
    [
        /// Activates the next pane in the workspace.
        ActivateNextPane,
        /// Activates the previous pane in the workspace.
        ActivatePreviousPane,
        /// Activates the last pane in the workspace.
        ActivateLastPane,
        /// Moves focus to the next major region of the window (editor, open
        /// panels, status bar), cycling and wrapping around. Intended as a
        /// discoverable, screen-reader-friendly way to navigate the window.
        FocusNextPart,
        /// Moves focus to the previous major region of the window. See
        /// [`FocusNextPart`].
        FocusPreviousPart,
        /// Switches to the next window.
        ActivateNextWindow,
        /// Switches to the previous window.
        ActivatePreviousWindow,
        /// Adds a folder to the current project.
        AddFolderToProject,
        /// Clears all bookmarks in the project.
        ClearBookmarks,
        /// Clears all notifications.
        ClearAllNotifications,
        /// Clears all navigation history, including forward/backward navigation, recently opened files, and recently closed tabs. **This action is irreversible**.
        ClearNavigationHistory,
        /// Closes the active dock.
        CloseActiveDock,
        /// Closes all docks.
        CloseAllDocks,
        /// Toggles all docks.
        ToggleAllDocks,
        /// Closes the current window.
        CloseWindow,
        /// Closes the current project.
        CloseProject,
        /// Opens the feedback dialog.
        Feedback,
        /// Follows the next collaborator in the session.
        FollowNextCollaborator,
        /// Moves the focused panel to the next position.
        MoveFocusedPanelToNextPosition,
        /// Creates a new file.
        NewFile,
        /// Creates a new file in a vertical split.
        NewFileSplitVertical,
        /// Creates a new file in a horizontal split.
        NewFileSplitHorizontal,
        /// Opens a new search.
        NewSearch,
        /// Opens a new window.
        NewWindow,
        /// Opens multiple files.
        OpenFiles,
        /// Opens the current location in terminal.
        OpenInTerminal,
        /// Opens the component preview.
        OpenComponentPreview,
        /// Reloads the active item.
        ReloadActiveItem,
        /// Reopens the most recently dismissed picker in the current window.
        ReopenLastPicker,
        /// Resets the active dock to its default size.
        ResetActiveDockSize,
        /// Resets all open docks to their default sizes.
        ResetOpenDocksSize,
        /// Resets all panes in the center group to equal sizes, preserving the split layout.
        ResetPaneSizes,
        /// Reloads the application
        Reload,
        /// Formats and saves the current file, regardless of the format_on_save setting.
        FormatAndSave,
        /// Saves the current file with a new name.
        SaveAs,
        /// Saves without formatting.
        SaveWithoutFormat,
        /// Shuts down all debug adapters.
        ShutdownDebugAdapters,
        /// Suppresses the current notification.
        SuppressNotification,
        /// Toggles the bottom dock.
        ToggleBottomDock,
        /// Toggles centered layout mode.
        ToggleCenteredLayout,
        /// Toggles edit prediction feature globally for all files.
        ToggleEditPrediction,
        /// Toggles the left dock.
        ToggleLeftDock,
        /// Toggles the right dock.
        ToggleRightDock,
        /// Toggles zoom on the active pane.
        ToggleZoom,
        /// Toggles maximizing the active editor pane within the center area,
        /// hiding other split panes but leaving docks/panels unaffected.
        ToggleEditorZoom,
        /// Toggles read-only mode for the active item (if supported by that item).
        ToggleReadOnlyFile,
        /// Zooms in on the active pane.
        ZoomIn,
        /// Zooms out of the active pane.
        ZoomOut,
        /// If any worktrees are in restricted mode, shows a modal with possible actions.
        /// If the modal is shown already, closes it without trusting any worktree.
        ToggleWorktreeSecurity,
        /// Clears all trusted worktrees, placing them in restricted mode on next open.
        /// Requires restart to take effect on already opened projects.
        ClearTrustedWorktrees,
        /// Stops following a collaborator.
        Unfollow,
        /// Restores the banner.
        RestoreBanner,
        /// Toggles expansion of the selected item.
        ToggleExpandItem,
    ]
);

/// Opens a new terminal with the specified working directory.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct OpenTerminal {
    pub working_directory: PathBuf,
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}
