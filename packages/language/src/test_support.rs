use crate::{*, LanguageServerBinary};
use gpui::{AsyncApp};

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub struct FakeLspAdapter {
    pub name: &'static str,
    pub initialization_options: Option<Value>,
    pub additional_initialization_options: HashMap<LanguageServerName, Value>,
    pub additional_workspace_configuration: HashMap<LanguageServerName, Value>,
    pub prettier_plugins: Vec<&'static str>,
    pub disk_based_diagnostics_progress_token: Option<String>,
    pub disk_based_diagnostics_sources: Vec<String>,
    pub language_server_binary: LanguageServerBinary,

    pub capabilities: lsp::ServerCapabilities,
    pub initializer: Option<Box<dyn 'static + Send + Sync + Fn(&mut lsp::FakeLanguageServer)>>,
    pub label_for_completion: Option<
        Box<
            dyn 'static
                + Send
                + Sync
                + Fn(&lsp::CompletionItem, &Arc<Language>) -> Option<CodeLabel>,
        >,
    >,
}

#[cfg(any(test, feature = "test-support"))]
impl Default for FakeLspAdapter {
    fn default() -> Self {
        Self {
            name: "the-fake-language-server",
            capabilities: lsp::LanguageServer::full_capabilities(),
            initializer: None,
            disk_based_diagnostics_progress_token: None,
            initialization_options: None,
            additional_initialization_options: HashMap::default(),
            additional_workspace_configuration: HashMap::default(),
            disk_based_diagnostics_sources: Vec::new(),
            prettier_plugins: Vec::new(),
            language_server_binary: LanguageServerBinary {
                path: "/the/fake/lsp/path".into(),
                arguments: vec![],
                env: Default::default(),
            },
            label_for_completion: None,
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl LspInstaller for FakeLspAdapter {
    type BinaryVersion = ();

    async fn fetch_latest_server_version(
        &self,
        _: &Arc<dyn LspAdapterDelegate>,
        _: bool,
        _: &mut AsyncApp,
    ) -> Result<Self::BinaryVersion> {
        unreachable!()
    }

    async fn check_if_user_installed(
        &self,
        _: &Arc<dyn LspAdapterDelegate>,
        _: Option<Toolchain>,
        _: &AsyncApp,
    ) -> Option<LanguageServerBinary> {
        Some(self.language_server_binary.clone())
    }

    fn fetch_server_binary(
        &self,
        _: (),
        _: PathBuf,
        _: &Arc<dyn LspAdapterDelegate>,
    ) -> impl Send + Future<Output = Result<LanguageServerBinary>> + use<> {
        async {
            unreachable!();
        }
    }

    async fn cached_server_binary(
        &self,
        _: PathBuf,
        _: &dyn LspAdapterDelegate,
    ) -> Option<LanguageServerBinary> {
        unreachable!();
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait(?Send)]
impl LspAdapter for FakeLspAdapter {
    fn name(&self) -> LanguageServerName {
        LanguageServerName(self.name.into())
    }

    fn disk_based_diagnostic_sources(&self) -> Vec<String> {
        self.disk_based_diagnostics_sources.clone()
    }

    fn disk_based_diagnostics_progress_token(&self) -> Option<String> {
        self.disk_based_diagnostics_progress_token.clone()
    }

    async fn initialization_options(
        self: Arc<Self>,
        _: &Arc<dyn LspAdapterDelegate>,
        _cx: &mut AsyncApp,
    ) -> Result<Option<Value>> {
        Ok(self.initialization_options.clone())
    }

    async fn additional_initialization_options(
        self: Arc<Self>,
        target_language_server_id: LanguageServerName,
        _: &Arc<dyn LspAdapterDelegate>,
    ) -> Result<Option<Value>> {
        Ok(self
            .additional_initialization_options
            .get(&target_language_server_id)
            .cloned())
    }

    async fn additional_workspace_configuration(
        self: Arc<Self>,
        target_language_server_id: LanguageServerName,
        _: &Arc<dyn LspAdapterDelegate>,
        _cx: &mut AsyncApp,
    ) -> Result<Option<Value>> {
        Ok(self
            .additional_workspace_configuration
            .get(&target_language_server_id)
            .cloned())
    }

    async fn label_for_completion(
        &self,
        item: &lsp::CompletionItem,
        language: &Arc<Language>,
    ) -> Option<CodeLabel> {
        let label_for_completion = self.label_for_completion.as_ref()?;
        label_for_completion(item, language)
    }

    fn is_extension(&self) -> bool {
        false
    }
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn rust_lang() -> Arc<Language> {
    test_language("rust", tree_sitter_rust::LANGUAGE.into())
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn json_lang() -> Arc<Language> {
    test_language("json", tree_sitter_json::LANGUAGE.into())
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn markdown_lang() -> Arc<Language> {
    test_language("markdown", tree_sitter_md::LANGUAGE.into())
}

#[cfg(any(test, feature = "test-support"))]
fn test_language(name: &str, grammar: tree_sitter::Language) -> Arc<Language> {
    Arc::new(
        Language::new(grammars::load_config(name), Some(grammar))
            .with_queries(grammars::load_queries(name))
            .unwrap_or_else(|error| panic!("could not parse queries for language {name}: {error}")),
    )
}
