use dap::inline_value::{InlineValueLocation, VariableLookupKind, VariableScope};
use language::DebuggerTextObject;
use std::collections::HashSet;

pub(crate) fn proto_to_prompt(level: proto::language_server_prompt_request::Level) -> gpui::PromptLevel {
    // 原样搬入
}

pub(crate) fn provide_inline_values(
    captures: impl Iterator<Item = (std::ops::Range<usize>, DebuggerTextObject)>,
    snapshot: &language::BufferSnapshot,
    max_row: usize,
) -> Vec<InlineValueLocation> {
    // 原样搬入
}
