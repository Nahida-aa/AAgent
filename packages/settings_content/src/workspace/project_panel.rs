use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

use crate::{FolderIndicator, ui::DockSide};

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct ProjectPanelSettingsContent {
    /// Whether to show the project panel button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,
    /// Whether to hide gitignore files in the project panel.
    ///
    /// Default: false
    pub hide_gitignore: Option<bool>,
    /// Customize default width (in pixels) taken by project panel
    ///
    /// Default: 240
    pub default_width: Option<crate::PixelSetting>,
    /// The position of project panel
    ///
    /// Default: right (Agentic layout), left (Classic layout)
    pub dock: Option<DockSide>,
    // TODO
    pub title_tooltip_delay: Option<ProjectPanelTitleTooltipDelay>,
    /// Spacing between worktree entries in the project panel.
    ///
    /// Default: comfortable
    pub entry_spacing: Option<ProjectPanelEntrySpacing>,
    /// Whether to show file icons in the project panel.
    ///
    /// Default: true
    pub file_icons: Option<bool>,
    /// What to show for directories in the project panel.
    ///
    /// Default: icon
    pub folder_indicator: Option<FolderIndicator>,
    /// Whether to show the git status in the project panel.
    ///
    /// Default: true
    pub git_status: Option<bool>,
    /// Amount of indentation (in pixels) for nested items.
    ///
    /// Default: 20
    pub indent_size: Option<crate::PixelSetting>,
    /// Whether to reveal it in the project panel automatically,
    /// when a corresponding project entry becomes active.
    /// Gitignored entries are never auto revealed.
    ///
    /// Default: true
    pub auto_reveal_entries: Option<bool>,
    /// Whether to fold directories automatically
    /// when directory has only one directory inside.
    ///
    /// Default: true
    pub auto_fold_dirs: Option<bool>,
    /// Whether to show folder names with bold text in the project panel.
    ///
    /// Default: false
    pub bold_folder_labels: Option<bool>,
    /// Whether the project panel should open on startup.
    ///
    /// Default: true
    pub starts_open: Option<bool>,
    /// Scrollbar-related settings
    pub scrollbar: Option<ProjectPanelScrollbarSettingsContent>,
    /// Which files containing diagnostic errors/warnings to mark in the project panel.
    ///
    /// Default: all
    pub show_diagnostics: Option<ShowDiagnostics>,
    /// Settings related to indent guides in the project panel.
    pub indent_guides: Option<ProjectPanelIndentGuidesSettings>,
    /// Whether to hide the root entry when only one folder is open in the window.
    ///
    /// Default: false
    pub hide_root: Option<bool>,
    /// Whether to hide the hidden entries in the project panel.
    ///
    /// Default: false
    pub hide_hidden: Option<bool>,
    /// Whether to stick parent directories at top of the project panel.
    ///
    /// Default: true
    pub sticky_scroll: Option<bool>,
    /// Whether to enable drag-and-drop operations in the project panel.
    ///
    /// Default: true
    pub drag_and_drop: Option<bool>,
    /// Settings for automatically opening files.
    pub auto_open: Option<ProjectPanelAutoOpenSettings>,
    /// How to order sibling entries in the project panel.
    ///
    /// Default: directories_first
    pub sort_mode: Option<ProjectPanelSortMode>,
    /// Whether to sort file and folder names case-sensitively in the project panel.
    /// This works in combination with `sort_mode`. `sort_mode` controls how files and
    /// directories are grouped, while this setting controls how names are compared.
    ///
    /// Default: default
    pub sort_order: Option<ProjectPanelSortOrder>,
    /// Whether to show error and warning count badges next to file names in the project panel.
    ///
    /// Default: false
    pub diagnostic_badges: Option<bool>,
    /// Whether to show a git status indicator next to file names in the project panel.
    ///
    /// Default: false
    pub git_status_indicator: Option<bool>,
}
