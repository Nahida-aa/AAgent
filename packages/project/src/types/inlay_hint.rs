use super::*;

pub enum InlayId {
    EditPrediction(usize),
    DebuggerValue(usize),
    // LSP
    Hint(usize),
    Color(usize),
    ReplResult(usize),
}

impl InlayId {
    pub fn id(&self) -> usize {
        match self {
            Self::EditPrediction(id) => *id,
            Self::DebuggerValue(id) => *id,
            Self::Hint(id) => *id,
            Self::Color(id) => *id,
            Self::ReplResult(id) => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlayHint {
    pub position: language::Anchor,
    pub label: InlayHintLabel,
    pub kind: Option<InlayHintKind>,
    pub padding_left: bool,
    pub padding_right: bool,
    pub tooltip: Option<InlayHintTooltip>,
    pub resolve_state: ResolveState,
}

/// The user's intent behind a given completion confirmation.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]

pub enum ResolveState {
    Resolved,
    CanResolve(LanguageServerId, Option<lsp::LSPAny>),
    Resolving,
}
impl InlayHint {
    pub fn text(&self) -> Rope {
        match &self.label {
            InlayHintLabel::String(s) => Rope::from(s),
            InlayHintLabel::LabelParts(parts) => parts.iter().map(|part| &*part.value).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]

pub enum InlayHintLabel {
    String(String),
    LabelParts(Vec<InlayHintLabelPart>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlayHintLabelPart {
    pub value: String,
    pub tooltip: Option<InlayHintLabelPartTooltip>,
    pub location: Option<(LanguageServerId, lsp::Location)>,
    pub command: Option<(LanguageServerId, lsp::Command)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlayHintTooltip {
    String(String),
    MarkupContent(MarkupContent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlayHintLabelPartTooltip {
    String(String),
    MarkupContent(MarkupContent),
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct MarkupContent {
    pub kind: HoverBlockKind,
    pub value: String,
}
