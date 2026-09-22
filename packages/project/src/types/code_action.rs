use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CodeAction {
    /// The id of the language server that produced this code action.
    pub server_id: LanguageServerId,
    /// The range of the buffer where this code action is applicable.
    pub range: Range<Anchor>,
    /// The raw code action provided by the language server.
    /// Can be either an action or a command.
    pub lsp_action: LspAction,
    /// Whether the action needs to be resolved using the language server.
    pub resolved: bool,
}

/// An action sent back by a language server.
#[derive(Clone, Debug, PartialEq)]
pub enum LspAction {
    /// An action with the full data, may have a command or may not.
    /// May require resolving.
    Action(Box<lsp::CodeAction>),
    /// A command data to run as an action.
    Command(lsp::Command),
    /// A code lens data to run as an action.
    CodeLens(lsp::CodeLens),
}

impl LspAction {
    pub fn title(&self) -> &str {
        match self {
            Self::Action(action) => &action.title,
            Self::Command(command) => &command.title,
            Self::CodeLens(lens) => lens
                .command
                .as_ref()
                .map(|command| command.title.as_str())
                .unwrap_or("Unknown command"),
        }
    }

    pub fn action_kind(&self) -> Option<lsp::CodeActionKind> {
        match self {
            Self::Action(action) => action.kind.clone(),
            Self::Command(_) => Some(lsp::CodeActionKind::new("command")),
            Self::CodeLens(_) => Some(lsp::CodeActionKind::new("code lens")),
        }
    }

    pub fn edit(&self) -> Option<&lsp::WorkspaceEdit> {
        match self {
            Self::Action(action) => action.edit.as_ref(),
            Self::Command(_) => None,
            Self::CodeLens(_) => None,
        }
    }

    pub fn command(&self) -> Option<&lsp::Command> {
        match self {
            Self::Action(action) => action.command.as_ref(),
            Self::Command(command) => Some(command),
            Self::CodeLens(lens) => lens.command.as_ref(),
        }
    }
}
