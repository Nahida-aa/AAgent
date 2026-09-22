use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use collections::{HashMap, HashSet};
use language::{Language, ResolvedHighlights, Rope};
use sum_tree::TreeMap;

use crate::html::html_parser::ParsedHtmlBlock;
use crate::mermaid::ParsedMarkdownMermaidDiagram;
use crate::parser::{
    CodeBlockKind, MarkdownEvent, MarkdownTag, MarkdownTagEnd, ParsedMetadataBlock,
};
use crate::selection;
use gpui::SharedString;

#[derive(Debug, Clone, Default)]
pub struct ParsedMarkdown {
    pub source: SharedString,
    pub events: Arc<[(Range<usize>, MarkdownEvent)]>,
    pub languages_by_name: TreeMap<SharedString, Arc<Language>>,
    pub languages_by_path: TreeMap<Arc<str>, Arc<Language>>,
    pub root_block_starts: Arc<[usize]>,
    pub(crate) html_blocks: BTreeMap<usize, crate::html::html_parser::ParsedHtmlBlock>,
    pub(crate) metadata_blocks: BTreeMap<usize, ParsedMetadataBlock>,
    pub(crate) mermaid_diagrams: BTreeMap<usize, ParsedMarkdownMermaidDiagram>,
    pub heading_slugs: HashMap<SharedString, usize>,
    pub footnote_definitions: HashMap<SharedString, usize>,
    pub(crate) link_definition_spans: Arc<[Range<usize>]>,
    pub(crate) fallback_code_block_language: Option<Arc<Language>>,
    pub(crate) code_block_highlights: Arc<CodeBlockHighlights>,
}

impl ParsedMarkdown {
    pub fn source(&self) -> &SharedString { &self.source }

    pub fn events(&self) -> &Arc<[(Range<usize>, MarkdownEvent)]> { &self.events }

    pub fn root_block_starts(&self) -> &Arc<[usize]> { &self.root_block_starts }

    pub(super) fn non_rendered_source_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut active_link = None;
        let mut active_image: Option<Range<usize>> = None;

        for (event_range, event) in self.events.iter() {
            // An image renders as an image, so nothing between its start and end is text on
            // screen - not the alt text, and not the destination. Skipping these events also
            // keeps the alt text from advancing the enclosing link's cursor, which would
            // otherwise carve it out of the link's non-rendered span.
            if let Some(image_range) = &active_image {
                if matches!(event, MarkdownEvent::End(MarkdownTagEnd::Image))
                    && event_range.end >= image_range.end
                {
                    active_image = None;
                }
                continue;
            }

            match event {
                MarkdownEvent::Start(MarkdownTag::Image { .. }) => {
                    active_image = Some(event_range.clone());
                    if active_link.is_none() {
                        ranges.push(event_range.clone());
                    }
                }
                MarkdownEvent::Start(MarkdownTag::Link { .. }) => {
                    active_link = Some((event_range.clone(), event_range.start));
                }
                MarkdownEvent::Text
                | MarkdownEvent::SubstitutedText(_)
                | MarkdownEvent::Code
                | MarkdownEvent::SubstitutedCode(_) => {
                    let Some((link_range, cursor)) = active_link.as_mut() else {
                        continue;
                    };
                    let visible_start = event_range.start.max(link_range.start);
                    let visible_end = event_range.end.min(link_range.end);
                    if visible_start > *cursor {
                        ranges.push(*cursor..visible_start);
                    }
                    *cursor = (*cursor).max(visible_end);
                }
                MarkdownEvent::End(MarkdownTagEnd::Link) => {
                    if let Some((link_range, cursor)) = active_link.take()
                        && cursor < link_range.end
                    {
                        ranges.push(cursor..link_range.end);
                    }
                }
                _ => {}
            }
        }

        ranges.extend(self.link_definition_spans.iter().cloned());
        // Callers binary search these ranges, which requires them to be sorted and disjoint.
        ranges.sort_by_key(|range| range.start);
        ranges.dedup_by(|next, previous| {
            if next.start <= previous.end {
                previous.end = previous.end.max(next.end);
                true
            } else {
                false
            }
        });
        ranges
    }

    pub fn root_block_for_source_index(&self, source_index: usize) -> Option<usize> {
        if self.root_block_starts.is_empty() {
            return None;
        }

        let partition = self
            .root_block_starts
            .partition_point(|block_start| *block_start <= source_index);

        Some(partition.saturating_sub(1))
    }

    /// Extracts the markdown source for a selection, rebalancing inline
    /// delimiters (`**`, backticks, link syntax, etc.) so partial selections of
    /// styled spans stay well-formed.
    ///
    /// With an exception of a single inline code span, which is returned as plain
    /// text, since copying a command or identifier is the dominant use case there.
    pub fn rebalanced_markdown_for_selection(&self, selection: Range<usize>) -> String {
        selection::rebalanced_markdown_for_selection(
            &self.source,
            &self.events,
            &self.root_block_starts,
            selection,
        )
    }

    pub(crate) fn code_block_language(&self, kind: &CodeBlockKind) -> Option<Arc<Language>> {
        match kind {
            CodeBlockKind::FencedLang(name) => self.languages_by_name.get(name).cloned(),
            CodeBlockKind::FencedSrc(path_range) => {
                self.languages_by_path.get(&path_range.path).cloned()
            }
            CodeBlockKind::Fenced => self.fallback_code_block_language.clone(),
            CodeBlockKind::Indented => None,
        }
    }
}

struct PendingCodeBlock<'a> {
    language: Arc<Language>,
    texts: Vec<(Range<usize>, &'a str)>,
}

pub(crate) fn compute_code_block_highlights(parsed: &ParsedMarkdown) -> CodeBlockHighlights {
    let mut code_block_highlights = CodeBlockHighlights::default();
    let mut pending_block: Option<PendingCodeBlock> = None;
    for (range, event) in parsed.events.iter() {
        match event {
            MarkdownEvent::Start(MarkdownTag::CodeBlock { kind, .. }) => {
                if parsed.mermaid_diagrams.contains_key(&range.start) {
                    pending_block = None;
                    continue;
                }
                pending_block = parsed
                    .code_block_language(kind)
                    .map(|language| PendingCodeBlock {
                        language,
                        texts: Vec::new(),
                    });
            }
            MarkdownEvent::End(MarkdownTagEnd::CodeBlock) => {
                if let Some(block) = pending_block.take() {
                    highlight_code_block(block, &mut code_block_highlights);
                }
            }
            MarkdownEvent::Text => {
                if let Some(block) = &mut pending_block {
                    block
                        .texts
                        .push((range.clone(), &parsed.source[range.clone()]));
                }
            }
            MarkdownEvent::SubstitutedText(text) => {
                if let Some(block) = &mut pending_block {
                    block.texts.push((range.clone(), text.as_str()));
                }
            }
            _ => {}
        }
    }
    code_block_highlights
}

fn highlight_code_block(block: PendingCodeBlock, code_block_highlights: &mut CodeBlockHighlights) {
    let mut combined = String::new();
    let mut text_offsets = Vec::with_capacity(block.texts.len());
    for (_, text) in &block.texts {
        text_offsets.push(combined.len());
        combined.push_str(text);
    }
    let resolved = block
        .language
        .highlight_text_resolved(&Rope::from(combined.as_str()), 0..combined.len());
    if resolved.runs.is_empty() {
        return;
    }
    if let [(source_range, _)] = block.texts.as_slice() {
        code_block_highlights.insert(source_range.start, resolved);
        return;
    }
    let mut runs = resolved.runs.iter().peekable();
    for ((source_range, text), text_offset) in block.texts.iter().zip(text_offsets) {
        let text_end = text_offset + text.len();
        let mut text_runs = Vec::new();
        while let Some((run_range, highlight_id)) = runs.peek() {
            if run_range.start >= text_end {
                break;
            }
            let start = run_range.start.max(text_offset);
            let end = run_range.end.min(text_end);
            if end > start {
                text_runs.push((start - text_offset..end - text_offset, *highlight_id));
            }
            if run_range.end > text_end {
                break;
            }
            runs.next();
        }
        if !text_runs.is_empty() {
            code_block_highlights.insert(
                source_range.start,
                ResolvedHighlights {
                    sources: resolved.sources.clone(),
                    runs: text_runs.into(),
                },
            );
        }
    }
}

pub(crate) type CodeBlockHighlights = HashMap<usize, ResolvedHighlights>;
