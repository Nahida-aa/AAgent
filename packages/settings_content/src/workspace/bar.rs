use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug, PartialEq)]
pub struct TabBarSettingsContent {
    /// Whether or not to show the tab bar in the editor.
    ///
    /// Default: true
    pub show: Option<bool>,
    /// Whether or not to show the navigation history buttons in the tab bar.
    ///
    /// Default: true
    pub show_nav_history_buttons: Option<bool>,
    /// Whether or not to show the tab bar buttons.
    ///
    /// Default: true
    pub show_tab_bar_buttons: Option<bool>,
    /// Whether or not to show pinned tabs in a separate row.
    /// When enabled, pinned tabs appear in a top row and unpinned tabs in a bottom row.
    ///
    /// Default: false
    pub show_pinned_tabs_in_separate_row: Option<bool>,
}

#[with_fallible_options]
#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug, PartialEq, Eq)]
pub struct StatusBarSettingsContent {
    /// Whether to show the status bar.
    ///
    /// Default: true
    #[serde(rename = "experimental.show")]
    pub show: Option<bool>,
    /// Whether to show the name of the active file in the status bar.
    ///
    /// Default: false
    pub show_active_file: Option<bool>,
    /// Whether to display the active language button in the status bar.
    ///
    /// Default: true
    pub active_language_button: Option<bool>,
    /// Whether to show the cursor position button in the status bar.
    ///
    /// Default: true
    pub cursor_position_button: Option<bool>,
    /// Whether to show active line endings button in the status bar.
    ///
    /// Default: false
    pub line_endings_button: Option<bool>,
    /// Whether to show the active encoding button in the status bar.
    ///
    /// Default: non_utf8
    pub active_encoding_button: Option<EncodingDisplayOptions>,
    /// Whether to show an indicator with a countdown while timed multi-stroke input is pending.
    /// Hovering the indicator pauses the timeout.
    /// Its binding preview popover is disabled when the which-key popup is enabled.
    ///
    /// Default: true
    pub pending_keystrokes_indicator: Option<bool>,
}

#[with_fallible_options]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct PreviewTabsSettingsContent {
    /// Whether to show opened editors as preview tabs.
    /// Preview tabs do not stay open, are reused until explicitly set to be kept open opened (via double-click or editing) and show file names in italic.
    ///
    /// Default: true
    pub enabled: Option<bool>,
    /// Whether to open tabs in preview mode when opened from the project panel
    /// with a single click or the `project_panel::Open` action.
    ///
    /// Default: true
    pub enable_preview_from_project_panel: Option<bool>,
    /// Whether to open tabs in preview mode when selected from the file finder.
    ///
    /// Default: false
    pub enable_preview_from_file_finder: Option<bool>,
    /// Whether to open tabs in preview mode when opened from a multibuffer.
    ///
    /// Default: true
    pub enable_preview_from_multibuffer: Option<bool>,
    /// Whether to open tabs in preview mode when code navigation is used to open a multibuffer.
    ///
    /// Default: false
    pub enable_preview_multibuffer_from_code_navigation: Option<bool>,
    /// Whether to open tabs in preview mode when code navigation is used to open a single file.
    ///
    /// Default: true
    pub enable_preview_file_from_code_navigation: Option<bool>,
    /// Whether to keep tabs in preview mode when code navigation is used to navigate away from them.
    /// If `enable_preview_file_from_code_navigation` or `enable_preview_multibuffer_from_code_navigation` is also true, the new tab may replace the existing one.
    ///
    /// Default: false
    pub enable_keep_preview_on_code_navigation: Option<bool>,
}
