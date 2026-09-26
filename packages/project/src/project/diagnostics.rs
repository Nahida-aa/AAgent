use crate::DiagnosticSummary;

use super::*;

impl Project {
    pub fn diagnostic_summary(&self, include_ignored: bool, cx: &App) -> DiagnosticSummary {
        self.lsp_store
            .read(cx)
            .diagnostic_summary(include_ignored, cx)
    }

    /// Returns a summary of the diagnostics for the provided project path only.
    pub fn diagnostic_summary_for_path(&self, path: &ProjectPath, cx: &App) -> DiagnosticSummary {
        self.lsp_store
            .read(cx)
            .diagnostic_summary_for_path(path, cx)
    }

    pub fn diagnostic_summaries<'a>(
        &'a self,
        include_ignored: bool,
        cx: &'a App,
    ) -> impl Iterator<Item = (ProjectPath, LanguageServerId, DiagnosticSummary)> + 'a {
        self.lsp_store
            .read(cx)
            .diagnostic_summaries(include_ignored, cx)
    }
}
