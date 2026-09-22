use std::borrow::Cow;
use std::str::FromStr;

use serde::Serialize;

/// Variables, available for use in [`TaskContext`] when a Zed's [`TaskTemplate`] gets resolved into a [`ResolvedTask`].
/// Name of the variable must be a valid shell variable identifier, which generally means that it is
/// a word  consisting only  of alphanumeric characters and underscores,
/// and beginning with an alphabetic character or an  underscore.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum VariableName {
    /// An absolute path of the currently opened file.
    File,
    /// A path of the currently opened file (relative to worktree root).
    RelativeFile,
    /// A path of the currently opened file's directory (relative to worktree root).
    RelativeDir,
    /// The currently opened filename.
    Filename,
    /// The path to a parent directory of a currently opened file.
    Dirname,
    /// Stem (filename without extension) of the currently opened file.
    Stem,
    /// An absolute path of the currently opened worktree, that contains the file.
    WorktreeRoot,
    /// A symbol text, that contains latest cursor/selection position.
    Symbol,
    /// A row with the latest cursor/selection position.
    Row,
    /// A column with the latest cursor/selection position.
    Column,
    /// Text from the latest selection.
    SelectedText,
    /// The language of the currently opened buffer (e.g., "Rust", "Python").
    Language,
    /// The symbol selected by the symbol tagging system, specifically the @run capture in a runnables.scm
    RunnableSymbol,
    /// Open a Picker to select a process ID to use in place
    /// Can only be used to debug configurations
    PickProcessId,
    /// An absolute path of the main (original) git worktree for the current repository.
    /// For normal checkouts, this equals the worktree root. For linked worktrees,
    /// this is the original repo's working directory.
    MainGitWorktree,
    /// Full SHA for the Git commit associated with the task context.
    GitSha,
    /// Short SHA for the Git commit associated with the task context.
    GitShaShort,
    /// Name of the Git repository associated with the task context.
    GitRepositoryName,
    /// Absolute path of the Git repository associated with the task context.
    GitRepositoryPath,
    /// Name of the Git ref (branch, remote ref, or tag) associated with the task context.
    GitRef,
    /// Custom variable, provided by the plugin or other external source.
    /// Will be printed with `CUSTOM_` prefix to avoid potential conflicts with other variables.
    Custom(Cow<'static, str>),
}

impl VariableName {
    /// Generates a `$VARIABLE`-like string value to be used in templates.
    pub fn template_value(&self) -> String { format!("${self}") }

    /// Generates a `"$VARIABLE"`-like string, to be used instead of `Self::template_value` when expanded value could contain spaces or special characters.
    pub fn template_value_with_whitespace(&self) -> String { format!("\"${self}\"") }
}

impl FromStr for VariableName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let without_prefix = s.strip_prefix(ZED_VARIABLE_NAME_PREFIX).ok_or(())?;
        let value = match without_prefix {
            "FILE" => Self::File,
            "FILENAME" => Self::Filename,
            "RELATIVE_FILE" => Self::RelativeFile,
            "RELATIVE_DIR" => Self::RelativeDir,
            "DIRNAME" => Self::Dirname,
            "STEM" => Self::Stem,
            "WORKTREE_ROOT" => Self::WorktreeRoot,
            "SYMBOL" => Self::Symbol,
            "RUNNABLE_SYMBOL" => Self::RunnableSymbol,
            "SELECTED_TEXT" => Self::SelectedText,
            "LANGUAGE" => Self::Language,
            "ROW" => Self::Row,
            "COLUMN" => Self::Column,
            "MAIN_GIT_WORKTREE" => Self::MainGitWorktree,
            "GIT_SHA" => Self::GitSha,
            "GIT_SHA_SHORT" => Self::GitShaShort,
            "GIT_REPOSITORY_NAME" => Self::GitRepositoryName,
            "GIT_REPOSITORY_PATH" => Self::GitRepositoryPath,
            "GIT_REF" => Self::GitRef,
            _ => {
                if let Some(custom_name) =
                    without_prefix.strip_prefix(ZED_CUSTOM_VARIABLE_NAME_PREFIX)
                {
                    Self::Custom(Cow::Owned(custom_name.to_owned()))
                } else {
                    return Err(());
                }
            }
        };
        Ok(value)
    }
}

/// A prefix that all [`VariableName`] variants are prefixed with when used in environment variables and similar template contexts.
pub const ZED_VARIABLE_NAME_PREFIX: &str = "ZED_";
pub(crate) const ZED_CUSTOM_VARIABLE_NAME_PREFIX: &str = "CUSTOM_";

impl std::fmt::Display for VariableName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "{ZED_VARIABLE_NAME_PREFIX}FILE"),
            Self::Filename => write!(f, "{ZED_VARIABLE_NAME_PREFIX}FILENAME"),
            Self::RelativeFile => write!(f, "{ZED_VARIABLE_NAME_PREFIX}RELATIVE_FILE"),
            Self::RelativeDir => write!(f, "{ZED_VARIABLE_NAME_PREFIX}RELATIVE_DIR"),
            Self::Dirname => write!(f, "{ZED_VARIABLE_NAME_PREFIX}DIRNAME"),
            Self::Stem => write!(f, "{ZED_VARIABLE_NAME_PREFIX}STEM"),
            Self::WorktreeRoot => write!(f, "{ZED_VARIABLE_NAME_PREFIX}WORKTREE_ROOT"),
            Self::Symbol => write!(f, "{ZED_VARIABLE_NAME_PREFIX}SYMBOL"),
            Self::Row => write!(f, "{ZED_VARIABLE_NAME_PREFIX}ROW"),
            Self::Column => write!(f, "{ZED_VARIABLE_NAME_PREFIX}COLUMN"),
            Self::SelectedText => write!(f, "{ZED_VARIABLE_NAME_PREFIX}SELECTED_TEXT"),
            Self::Language => write!(f, "{ZED_VARIABLE_NAME_PREFIX}LANGUAGE"),
            Self::RunnableSymbol => write!(f, "{ZED_VARIABLE_NAME_PREFIX}RUNNABLE_SYMBOL"),
            Self::PickProcessId => write!(f, "{ZED_VARIABLE_NAME_PREFIX}PICK_PID"),
            Self::MainGitWorktree => write!(f, "{ZED_VARIABLE_NAME_PREFIX}MAIN_GIT_WORKTREE"),
            Self::GitSha => write!(f, "{ZED_VARIABLE_NAME_PREFIX}GIT_SHA"),
            Self::GitShaShort => write!(f, "{ZED_VARIABLE_NAME_PREFIX}GIT_SHA_SHORT"),
            Self::GitRepositoryName => write!(f, "{ZED_VARIABLE_NAME_PREFIX}GIT_REPOSITORY_NAME"),
            Self::GitRepositoryPath => write!(f, "{ZED_VARIABLE_NAME_PREFIX}GIT_REPOSITORY_PATH"),
            Self::GitRef => write!(f, "{ZED_VARIABLE_NAME_PREFIX}GIT_REF"),
            Self::Custom(s) => write!(
                f,
                "{ZED_VARIABLE_NAME_PREFIX}{ZED_CUSTOM_VARIABLE_NAME_PREFIX}{s}"
            ),
        }
    }
}
