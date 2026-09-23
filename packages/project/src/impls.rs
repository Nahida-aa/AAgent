use anyhow::Result;
use gpui::{App, AppContext as _, Entity, Hsla, Task};
use language::{Buffer, Capability, DiskState};
use lsp::CompletionItemKind;
use worktree::{File, ProjectEntryId};

use crate::Project;
use crate::color_extractor;
use crate::item::ProjectItem;
use crate::path::ProjectPath;
use crate::types::{Completion, CompletionSource};

impl ProjectItem for Buffer {
    fn try_open(
        project: &Entity<Project>,
        path: &ProjectPath,
        cx: &mut App,
    ) -> Option<Task<Result<Entity<Self>>>> {
        Some(project.update(cx, |project, cx| project.open_buffer(path.clone(), cx)))
    }

    fn entry_id(&self, _cx: &App) -> Option<ProjectEntryId> {
        File::from_dyn(self.file()).and_then(|file| file.project_entry_id())
    }

    fn project_path(&self, cx: &App) -> Option<ProjectPath> {
        let file = self.file()?;

        (!matches!(file.disk_state(), DiskState::Historic { .. })).then(|| ProjectPath {
            worktree_id: file.worktree_id(cx),
            path: file.path().clone(),
        })
    }

    fn is_dirty(&self) -> bool { self.is_dirty() }
}
impl Completion {
    pub fn kind(&self) -> Option<CompletionItemKind> {
        self.source
            // `lsp::CompletionListItemDefaults` has no `kind` field
            .lsp_completion(false)
            .and_then(|lsp_completion| lsp_completion.kind)
    }

    pub fn label(&self) -> Option<String> {
        self.source
            .lsp_completion(false)
            .map(|lsp_completion| lsp_completion.label.clone())
    }

    pub fn filter_text(&self) -> &str {
        self.source
            .filter_text()
            .unwrap_or_else(|| self.label.filter_text())
    }

    /// A key that can be used to sort completions when displaying
    /// them to the user.
    pub fn sort_key(&self) -> (usize, &str) {
        const DEFAULT_KIND_KEY: usize = 4;
        let kind_key = self
            .kind()
            .and_then(|lsp_completion_kind| match lsp_completion_kind {
                lsp::CompletionItemKind::KEYWORD => Some(0),
                lsp::CompletionItemKind::VARIABLE => Some(1),
                lsp::CompletionItemKind::CONSTANT => Some(2),
                lsp::CompletionItemKind::PROPERTY => Some(3),
                _ => None,
            })
            .unwrap_or(DEFAULT_KIND_KEY);
        (kind_key, self.label.filter_text())
    }

    /// Whether this completion is a snippet.
    pub fn is_snippet_kind(&self) -> bool {
        matches!(
            &self.source,
            CompletionSource::Lsp { lsp_completion, .. }
            if lsp_completion.kind == Some(CompletionItemKind::SNIPPET)
        )
    }

    /// Whether this completion is a snippet or snippet-style LSP completion.
    pub fn is_snippet(&self) -> bool {
        self.source
            // `lsp::CompletionListItemDefaults` has `insert_text_format` field
            .lsp_completion(true)
            .is_some_and(|lsp_completion| {
                lsp_completion.insert_text_format == Some(lsp::InsertTextFormat::SNIPPET)
            })
    }

    /// Returns the corresponding color for this completion.
    ///
    /// Will return `None` if this completion's kind is not [`CompletionItemKind::COLOR`].
    pub fn color(&self) -> Option<Hsla> {
        // `lsp::CompletionListItemDefaults` has no `kind` field
        let lsp_completion = self.source.lsp_completion(false)?;
        if lsp_completion.kind? == CompletionItemKind::COLOR {
            return color_extractor::extract_color(&lsp_completion);
        }
        None
    }
}
