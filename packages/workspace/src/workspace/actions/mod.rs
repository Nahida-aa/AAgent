use gpui::Action;
use schemars::JsonSchema;
use serde::Deserialize;

mod workspace;
mod pane;
mod item;
mod dock;
mod collab;
mod zed;




actions!(
    project_symbols,
    [
        /// Toggles the project symbols search.
        #[action(name = "Toggle")]
        ToggleProjectSymbols
    ]
);

/// Toggles the file finder interface.
#[derive(Default, PartialEq, Eq, Clone, Deserialize, JsonSchema, Action)]
#[action(namespace = file_finder, name = "Toggle")]
#[serde(deny_unknown_fields)]
pub struct ToggleFileFinder {
    #[serde(default)]
    pub separate_history: bool,
    #[serde(default)]
    pub include_ignored: Option<bool>,
}

/// Opens a new terminal in the center.
#[derive(Default, PartialEq, Eq, Clone, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewCenterTerminal {
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}

/// Opens a new terminal.
#[derive(Default, PartialEq, Eq, Clone, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct NewTerminal {
    /// If true, creates a local terminal even in remote projects.
    #[serde(default)]
    pub local: bool,
}

/// Increases size of a currently focused dock by a given amount of pixels.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct IncreaseActiveDockSize {
    /// For 0px parameter, uses UI font size value.
    #[serde(default)]
    pub px: u32,
}

/// Decreases size of a currently focused dock by a given amount of pixels.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct DecreaseActiveDockSize {
    /// For 0px parameter, uses UI font size value.
    #[serde(default)]
    pub px: u32,
}

/// Increases size of all currently visible docks uniformly, by a given amount of pixels.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct IncreaseOpenDocksSize {
    /// For 0px parameter, uses UI font size value.
    #[serde(default)]
    pub px: u32,
}

/// Decreases size of all currently visible docks uniformly, by a given amount of pixels.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = workspace)]
#[serde(deny_unknown_fields)]
pub struct DecreaseOpenDocksSize {
    /// For 0px parameter, uses UI font size value.
    #[serde(default)]
    pub px: u32,
}

actions!(
    workspace,
    [
        /// Activates the pane to the left.
        ActivatePaneLeft,
        /// Activates the pane to the right.
        ActivatePaneRight,
        /// Activates the pane above.
        ActivatePaneUp,
        /// Activates the pane below.
        ActivatePaneDown,
        /// Swaps the current pane with the one to the left.
        SwapPaneLeft,
        /// Swaps the current pane with the one to the right.
        SwapPaneRight,
        /// Swaps the current pane with the one above.
        SwapPaneUp,
        /// Swaps the current pane with the one below.
        SwapPaneDown,
        // Swaps the current pane with the first available adjacent pane (searching in order: below, above, right, left) and activates that pane.
        SwapPaneAdjacent,
        /// Move the current pane to be at the far left.
        MovePaneLeft,
        /// Move the current pane to be at the far right.
        MovePaneRight,
        /// Move the current pane to be at the very top.
        MovePaneUp,
        /// Move the current pane to be at the very bottom.
        MovePaneDown,
    ]
);

actions!(
    zed,
    [
        /// Opens the Zed log file.
        OpenLog,
        /// Reveals the Zed log file in the system file manager.
        RevealLogInFileManager
    ]
);
