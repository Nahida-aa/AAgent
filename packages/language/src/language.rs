use collections::HashSet;
use lsp::LanguageServerName;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Range;
use std::sync::Arc;

use aa_gpui_kit_theme::SyntaxTheme;
use anyhow::Result;
use language_core::{
    BlockCommentConfig, BracketPair, CodeLabel, Grammar, HighlightId, LanguageConfig,
    LanguageConfigOverride, LanguageId, LanguageMatcher, OrderedListConfig, Override, SoftWrap,
    Symbol, TaskListConfig,
    highlight_cache::{MAX_TEXT_HIGHLIGHT_ENTRY_BYTES, ResolvedHighlights, TextHighlightKey},
    highlight_map::HighlightMap,
};
use regex::Regex;
use text::Rope;
use tree_sitter::Tree;

use crate::LanguageName;
use crate::language_registry::LanguageQueries;
use crate::manifest::ManifestName;
use crate::parser_pool::parse_text;
use crate::syntax_map::{SyntaxSnapshot, flattened_highlight_regions};
use crate::task_context::ContextProvider;
use crate::toolchain::ToolchainLister;

pub struct Language {
    pub(crate) id: LanguageId,
    pub(crate) config: LanguageConfig,
    pub(crate) grammar: Option<Arc<Grammar>>,
    pub(crate) context_provider: Option<Arc<dyn ContextProvider>>,
    pub(crate) toolchain: Option<Arc<dyn ToolchainLister>>,
    pub(crate) manifest_name: Option<ManifestName>,
}

impl Language {
    pub fn new(config: LanguageConfig, ts_language: Option<tree_sitter::Language>) -> Self {
        Self::new_with_id(LanguageId::new(), config, ts_language)
    }

    pub fn id(&self) -> LanguageId { self.id }

    pub(super) fn new_with_id(
        id: LanguageId,
        config: LanguageConfig,
        ts_language: Option<tree_sitter::Language>,
    ) -> Self {
        Self {
            id,
            config,
            grammar: ts_language.map(|ts_language| Arc::new(Grammar::new(ts_language))),
            context_provider: None,
            toolchain: None,
            manifest_name: None,
        }
    }

    pub fn with_context_provider(mut self, provider: Option<Arc<dyn ContextProvider>>) -> Self {
        self.context_provider = provider;
        self
    }

    pub fn with_toolchain_lister(mut self, provider: Option<Arc<dyn ToolchainLister>>) -> Self {
        self.toolchain = provider;
        self
    }

    pub fn with_manifest(mut self, name: Option<ManifestName>) -> Self {
        self.manifest_name = name;
        self
    }

    pub fn with_queries(mut self, queries: LanguageQueries) -> Result<Self> {
        if let Some(grammar) = self.grammar.take() {
            let grammar =
                Arc::try_unwrap(grammar).map_err(|_| anyhow::anyhow!("cannot mutate grammar"))?;
            let grammar = grammar.with_queries(queries, &mut self.config)?;
            self.grammar = Some(Arc::new(grammar));
        }
        Ok(self)
    }

    pub fn with_highlights_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query(|grammar| grammar.with_highlights_query(source))
    }

    pub fn with_runnable_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query(|grammar| grammar.with_runnable_query(source))
    }

    pub fn with_outline_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| grammar.with_outline_query(source, name))
    }

    pub fn with_text_object_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| {
            grammar.with_text_object_query(source, name)
        })
    }

    pub fn with_debug_variables_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| {
            grammar.with_debug_variables_query(source, name)
        })
    }

    pub fn with_brackets_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| grammar.with_brackets_query(source, name))
    }

    pub fn with_indents_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| grammar.with_indents_query(source, name))
    }

    pub fn with_injection_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| grammar.with_injection_query(source, name))
    }

    pub fn with_override_query(mut self, source: &str) -> Result<Self> {
        if let Some(grammar_arc) = self.grammar.take() {
            let grammar = Arc::try_unwrap(grammar_arc)
                .map_err(|_| anyhow::anyhow!("cannot mutate grammar"))?;
            let grammar = grammar.with_override_query(
                source,
                &self.config.name,
                &self.config.overrides,
                &mut self.config.brackets,
                &self.config.scope_opt_in_language_servers,
            )?;
            self.grammar = Some(Arc::new(grammar));
        }
        Ok(self)
    }

    pub fn with_redaction_query(self, source: &str) -> Result<Self> {
        self.with_grammar_query_and_name(|grammar, name| grammar.with_redaction_query(source, name))
    }

    fn with_grammar_query(
        mut self,
        build: impl FnOnce(Grammar) -> Result<Grammar>,
    ) -> Result<Self> {
        if let Some(grammar_arc) = self.grammar.take() {
            let grammar = Arc::try_unwrap(grammar_arc)
                .map_err(|_| anyhow::anyhow!("cannot mutate grammar"))?;
            self.grammar = Some(Arc::new(build(grammar)?));
        }
        Ok(self)
    }

    fn with_grammar_query_and_name(
        mut self,
        build: impl FnOnce(Grammar, &LanguageName) -> Result<Grammar>,
    ) -> Result<Self> {
        if let Some(grammar_arc) = self.grammar.take() {
            let grammar = Arc::try_unwrap(grammar_arc)
                .map_err(|_| anyhow::anyhow!("cannot mutate grammar"))?;
            self.grammar = Some(Arc::new(build(grammar, &self.config.name)?));
        }
        Ok(self)
    }

    pub fn name(&self) -> LanguageName { self.config.name.clone() }
    pub fn manifest(&self) -> Option<&ManifestName> { self.manifest_name.as_ref() }

    pub fn code_fence_block_name(&self) -> Arc<str> {
        self.config
            .code_fence_block_name
            .clone()
            .unwrap_or_else(|| self.config.name.as_ref().to_lowercase().into())
    }

    pub fn matches_kernel_language(&self, kernel_language: &str) -> bool {
        let kernel_language_lower = kernel_language.to_lowercase();

        if self.code_fence_block_name().to_lowercase() == kernel_language_lower {
            return true;
        }

        if self.config.name.as_ref().to_lowercase() == kernel_language_lower {
            return true;
        }

        self.config
            .kernel_language_names
            .iter()
            .any(|name| name.to_lowercase() == kernel_language_lower)
    }

    pub fn context_provider(&self) -> Option<Arc<dyn ContextProvider>> {
        self.context_provider.clone()
    }

    pub fn toolchain_lister(&self) -> Option<Arc<dyn ToolchainLister>> { self.toolchain.clone() }

    pub fn highlight_text<'a>(
        self: &'a Arc<Self>,
        text: &'a Rope,
        range: Range<usize>,
    ) -> Vec<(Range<usize>, HighlightId)> {
        self.highlight_text_resolved(text, range).runs.to_vec()
    }

    pub fn highlight_text_resolved(
        self: &Arc<Self>,
        text: &Rope,
        range: Range<usize>,
    ) -> ResolvedHighlights {
        let Some(grammar) = &self.grammar else {
            return ResolvedHighlights::default();
        };
        let Some(highlights_config) = &grammar.highlights_config else {
            return ResolvedHighlights::default();
        };
        let highlights = if text.len() > MAX_TEXT_HIGHLIGHT_ENTRY_BYTES {
            self.compute_resolved_highlights(grammar, text)
        } else {
            let key = TextHighlightKey::new(text.chunks(), text.len());
            match highlights_config
                .text_highlight_cache
                .get(&key, text.chunks())
            {
                Some(highlights) => highlights,
                None => highlights_config.text_highlight_cache.insert(
                    key,
                    Arc::from(text.chunks().collect::<String>()),
                    self.compute_resolved_highlights(grammar, text),
                ),
            }
        };
        if range.start == 0 && range.end >= text.len() {
            return highlights;
        }
        ResolvedHighlights {
            sources: highlights.sources.clone(),
            runs: highlights
                .runs
                .iter()
                .filter(|(run_range, _)| run_range.start < range.end && run_range.end > range.start)
                .map(|(run_range, highlight_id)| {
                    (
                        run_range.start.max(range.start) - range.start
                            ..run_range.end.min(range.end) - range.start,
                        *highlight_id,
                    )
                })
                .collect(),
        }
    }

    fn compute_resolved_highlights(
        self: &Arc<Self>,
        grammar: &Arc<Grammar>,
        text: &Rope,
    ) -> ResolvedHighlights {
        let highlight_map = grammar.highlight_map();
        let tree = parse_text(grammar, text, None);
        let captures =
            SyntaxSnapshot::single_tree_captures(0..text.len(), text, &tree, self, |grammar| {
                grammar
                    .highlights_config
                    .as_ref()
                    .map(|config| &config.query)
            });
        let mut runs = Vec::<(Range<usize>, HighlightId)>::new();
        for region in flattened_highlight_regions(captures, 0..text.len()) {
            let highlight_id = region
                .stack
                .iter()
                .rev()
                .find_map(|capture| highlight_map.get(capture.capture_id));
            let Some(highlight_id) = highlight_id else {
                continue;
            };
            match runs.last_mut() {
                Some((last_range, last_highlight_id))
                    if *last_highlight_id == highlight_id
                        && last_range.end == region.range.start =>
                {
                    last_range.end = region.range.end;
                }
                _ => runs.push((region.range, highlight_id)),
            }
        }
        ResolvedHighlights {
            sources: [(Arc::clone(grammar), highlight_map)].into_iter().collect(),
            runs: runs.into(),
        }
    }

    pub fn path_suffixes(&self) -> &[String] { &self.config.matcher.path_suffixes }

    pub fn should_autoclose_before(&self, c: char) -> bool {
        c.is_whitespace() || self.config.autoclose_before.contains(c)
    }

    pub fn set_theme(&self, theme: &SyntaxTheme) {
        if let Some(grammar) = self.grammar.as_ref()
            && let Some(highlights_config) = &grammar.highlights_config
        {
            *grammar.highlight_map.lock() =
                build_highlight_map(highlights_config.query.capture_names(), theme);
        }
    }

    pub fn grammar(&self) -> Option<&Arc<Grammar>> { self.grammar.as_ref() }

    pub fn default_scope(self: &Arc<Self>) -> LanguageScope {
        LanguageScope {
            language: self.clone(),
            override_id: None,
        }
    }

    pub fn lsp_id(&self) -> String { self.config.name.lsp_id() }

    pub fn snippet_scope_id(&self) -> String { self.config.name.snippet_scope_id() }

    pub fn prettier_parser_name(&self) -> Option<&str> {
        self.config.prettier_parser_name.as_deref()
    }

    pub fn config(&self) -> &LanguageConfig { &self.config }
}

#[inline]
pub fn build_highlight_map(capture_names: &[&str], theme: &SyntaxTheme) -> HighlightMap {
    HighlightMap::from_ids(
        capture_names
            .iter()
            .map(|capture_name| theme.highlight_id(capture_name).map(HighlightId::new)),
    )
}

/// Represents a language for the given range. Some languages (e.g. HTML)
/// interleave several languages together, thus a single buffer might actually contain
/// several nested scopes.
#[derive(Clone, Debug)]
pub struct LanguageScope {
    language: Arc<Language>,
    override_id: Option<u32>,
}
impl LanguageScope {
    pub fn path_suffixes(&self) -> &[String] { self.language.path_suffixes() }

    pub fn language_name(&self) -> LanguageName { self.language.config.name.clone() }

    pub fn collapsed_placeholder(&self) -> &str {
        self.language.config.collapsed_placeholder.as_ref()
    }

    /// Returns line prefix that is inserted in e.g. line continuations or
    /// in `toggle comments` action.
    pub fn line_comment_prefixes(&self) -> &[Arc<str>] {
        Override::as_option(
            self.config_override().map(|o| &o.line_comments),
            Some(&self.language.config.line_comments),
        )
        .map_or([].as_slice(), |e| e.as_slice())
    }

    /// Config for block comments for this language.
    pub fn block_comment(&self) -> Option<&BlockCommentConfig> {
        Override::as_option(
            self.config_override().map(|o| &o.block_comment),
            self.language.config.block_comment.as_ref(),
        )
    }

    /// Config for documentation-style block comments for this language.
    pub fn documentation_comment(&self) -> Option<&BlockCommentConfig> {
        self.language.config.documentation_comment.as_ref()
    }

    /// Returns list markers that are inserted unchanged on newline (e.g., `- `, `* `, `+ `).
    pub fn unordered_list(&self) -> &[Arc<str>] { &self.language.config.unordered_list }

    /// Returns configuration for ordered lists with auto-incrementing numbers (e.g., `1. ` becomes `2. `).
    pub fn ordered_list(&self) -> &[OrderedListConfig] { &self.language.config.ordered_list }

    /// Returns configuration for task list continuation, if any (e.g., `- [x] ` continues as `- [ ] `).
    pub fn task_list(&self) -> Option<&TaskListConfig> { self.language.config.task_list.as_ref() }

    /// Returns additional regex patterns that act as prefix markers for creating
    /// boundaries during rewrapping.
    ///
    /// By default, Zed treats as paragraph and comment prefixes as boundaries.
    pub fn rewrap_prefixes(&self) -> &[Regex] { &self.language.config.rewrap_prefixes }

    /// Returns a list of language-specific word characters.
    ///
    /// By default, Zed treats alphanumeric characters (and '_') as word characters for
    /// the purpose of actions like 'move to next word end` or whole-word search.
    /// It additionally accounts for language's additional word characters.
    pub fn word_characters(&self) -> Option<&HashSet<char>> {
        Override::as_option(
            self.config_override().map(|o| &o.word_characters),
            Some(&self.language.config.word_characters),
        )
    }

    /// Returns a list of language-specific characters that are considered part of
    /// a completion query.
    pub fn completion_query_characters(&self) -> Option<&HashSet<char>> {
        Override::as_option(
            self.config_override()
                .map(|o| &o.completion_query_characters),
            Some(&self.language.config.completion_query_characters),
        )
    }

    /// Returns a list of language-specific characters that are considered part of
    /// identifiers during linked editing operations.
    pub fn linked_edit_characters(&self) -> Option<&HashSet<char>> {
        Override::as_option(
            self.config_override().map(|o| &o.linked_edit_characters),
            Some(&self.language.config.linked_edit_characters),
        )
    }

    /// Returns whether to prefer snippet `label` over `new_text` to replace text when
    /// completion is accepted.
    ///
    /// In cases like when cursor is in string or renaming existing function,
    /// you don't want to expand function signature instead just want function name
    /// to replace existing one.
    pub fn prefers_label_for_snippet_in_completion(&self) -> bool {
        self.config_override()
            .and_then(|o| o.prefer_label_for_snippet)
            .unwrap_or(false)
    }

    /// Returns a list of bracket pairs for a given language with an additional
    /// piece of information about whether the particular bracket pair is currently active for a given language.
    pub fn brackets(&self) -> impl Iterator<Item = (&BracketPair, bool)> {
        let mut disabled_ids = self
            .config_override()
            .map_or(&[] as _, |o| o.disabled_bracket_ixs.as_slice());
        self.language
            .config
            .brackets
            .pairs
            .iter()
            .enumerate()
            .map(move |(ix, bracket)| {
                let mut is_enabled = true;
                if let Some(next_disabled_ix) = disabled_ids.first()
                    && ix == *next_disabled_ix as usize
                {
                    disabled_ids = &disabled_ids[1..];
                    is_enabled = false;
                }
                (bracket, is_enabled)
            })
    }

    pub fn should_autoclose_before(&self, c: char) -> bool {
        c.is_whitespace() || self.language.config.autoclose_before.contains(c)
    }

    pub fn language_allowed(&self, name: &LanguageServerName) -> bool {
        let config = &self.language.config;
        let opt_in_servers = &config.scope_opt_in_language_servers;
        if opt_in_servers.contains(&name.0) {
            if let Some(over) = self.config_override() {
                over.opt_into_language_servers.contains(&name.0)
            } else {
                false
            }
        } else {
            true
        }
    }

    pub fn override_name(&self) -> Option<&str> {
        let id = self.override_id?;
        let grammar = self.language.grammar.as_ref()?;
        let override_config = grammar.override_config.as_ref()?;
        override_config.values.get(&id).map(|e| e.name.as_str())
    }

    fn config_override(&self) -> Option<&LanguageConfigOverride> {
        let id = self.override_id?;
        let grammar = self.language.grammar.as_ref()?;
        let override_config = grammar.override_config.as_ref()?;
        override_config.values.get(&id).map(|e| &e.value)
    }
}

impl Hash for Language {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.id.hash(state) }
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool { self.id.eq(&other.id) }
}

impl Eq for Language {}

impl Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Language")
            .field("name", &self.config.name)
            .finish()
    }
}
pub trait CodeLabelExt {
    fn fallback_for_completion(
        item: &lsp::CompletionItem,
        language: Option<&Language>,
    ) -> CodeLabel;
}

impl CodeLabelExt for CodeLabel {
    fn fallback_for_completion(
        item: &lsp::CompletionItem,
        language: Option<&Language>,
    ) -> CodeLabel {
        let highlight_id = item.kind.and_then(|kind| {
            let grammar = language?.grammar()?;
            use lsp::CompletionItemKind as Kind;
            match kind {
                Kind::CLASS => grammar.highlight_id_for_name("type"),
                Kind::CONSTANT => grammar.highlight_id_for_name("constant"),
                Kind::CONSTRUCTOR => grammar.highlight_id_for_name("constructor"),
                Kind::ENUM => grammar
                    .highlight_id_for_name("enum")
                    .or_else(|| grammar.highlight_id_for_name("type")),
                Kind::ENUM_MEMBER => grammar
                    .highlight_id_for_name("variant")
                    .or_else(|| grammar.highlight_id_for_name("property")),
                Kind::FIELD => grammar.highlight_id_for_name("property"),
                Kind::FUNCTION => grammar.highlight_id_for_name("function"),
                Kind::INTERFACE => grammar.highlight_id_for_name("type"),
                Kind::METHOD => grammar
                    .highlight_id_for_name("function.method")
                    .or_else(|| grammar.highlight_id_for_name("function")),
                Kind::OPERATOR => grammar.highlight_id_for_name("operator"),
                Kind::PROPERTY => grammar.highlight_id_for_name("property"),
                Kind::STRUCT => grammar.highlight_id_for_name("type"),
                Kind::VARIABLE => grammar.highlight_id_for_name("variable"),
                Kind::KEYWORD => grammar.highlight_id_for_name("keyword"),
                _ => None,
            }
        });

        let label = &item.label;
        let label_length = label.len();
        let runs = highlight_id
            .map(|highlight_id| vec![(0..label_length, highlight_id)])
            .unwrap_or_default();
        let text = if let Some(detail) = item.detail.as_deref().filter(|detail| detail != label) {
            format!("{label} {detail}")
        } else if let Some(description) = item
            .label_details
            .as_ref()
            .and_then(|label_details| label_details.description.as_deref())
            .filter(|description| description != label)
        {
            format!("{label} {description}")
        } else {
            label.clone()
        };
        let filter_range = item
            .filter_text
            .as_deref()
            .and_then(|filter| text.find(filter).map(|ix| ix..ix + filter.len()))
            .unwrap_or(0..label_length);
        CodeLabel {
            text,
            runs,
            filter_range,
        }
    }
}
