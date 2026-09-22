use lsp::CompletionContext;

pub const MAX_PROJECT_SEARCH_HISTORY_SIZE: usize = 500;

pub const CURRENT_PROJECT_FEATURES: &[&str] = &["new-style-anchors"];

#[cfg(feature = "test-support")]
pub const DEFAULT_COMPLETION_CONTEXT: CompletionContext = CompletionContext {
    trigger_kind: lsp::CompletionTriggerKind::INVOKED,
    trigger_character: None,
};
