use super::*;

#[derive(Clone, Debug, Default)]
pub enum LspPullDiagnostics {
    #[default]
    Default,
    Response {
        /// The id of the language server that produced diagnostics.
        server_id: LanguageServerId,
        /// URI of the resource,
        uri: lsp::Uri,
        /// The ID provided by the dynamic registration that produced diagnostics.
        registration_id: Option<SharedString>,
        /// The diagnostics produced by this language server.
        diagnostics: PulledDiagnostics,
    },
}

#[derive(Clone, Debug)]
pub enum PulledDiagnostics {
    Unchanged {
        /// An ID the current pulled batch for this file.
        /// If given, can be used to query workspace diagnostics partially.
        result_id: SharedString,
    },
    Changed {
        result_id: Option<SharedString>,
        diagnostics: Vec<lsp::Diagnostic>,
    },
}
