use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

#[with_fallible_options]
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct ItemSettingsContent {
    /// Whether to show the Git file status on a tab item.
    ///
    /// Default: false
    pub git_status: Option<bool>,
    /// Position of the close button in a tab.
    ///
    /// Default: right
    pub close_position: Option<ClosePosition>,
    /// Whether to show the file icon for a tab.
    ///
    /// Default: false
    pub file_icons: Option<bool>,
    /// What to do after closing the current tab.
    ///
    /// Default: history
    pub activate_on_close: Option<ActivateOnClose>,
    /// Which files containing diagnostic errors/warnings to mark in the tabs.
    /// This setting can take the following three values:
    ///
    /// Default: off
    pub show_diagnostics: Option<ShowDiagnostics>,
    /// Whether to always show the close button on tabs.
    ///
    /// Default: false
    pub show_close_button: Option<ShowCloseButton>,
}
