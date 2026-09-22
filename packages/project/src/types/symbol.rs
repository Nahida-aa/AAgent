use super::*;

pub struct Symbol {
    pub language_server_name: LanguageServerName,
    pub source_worktree_id: WorktreeId,
    pub source_language_server_id: LanguageServerId,
    pub path: SymbolLocation,
    pub label: CodeLabel,
    pub name: String,
    pub kind: language::SymbolKind,
    pub range: Range<Unclipped<PointUtf16>>,
    pub container_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: language::SymbolKind,
    pub range: Range<Unclipped<PointUtf16>>,
    pub selection_range: Range<Unclipped<PointUtf16>>,
    pub children: Vec<DocumentSymbol>,
}
