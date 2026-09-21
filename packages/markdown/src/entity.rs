use base64::Engine as _;
use collections::{HashMap, HashSet};
use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, Hsla, Image, ImageFormat, Pixels,
    Point, ScrollHandle, SharedString, Subscription, Task, Window, actions, point, px, size,
};
use language::{Language, LanguageName, LanguageRegistry, Rope};
use log::Level;
use settings::Settings as _;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use theme_settings::ThemeSettings;
use util::ResultExt;

use crate::mermaid::{
    MermaidState, ParsedMarkdownMermaidDiagram, extract_mermaid_diagrams, render_mermaid_diagram,
};
use crate::parsed::{ParsedMarkdown, compute_code_block_highlights};
use crate::parser::{
    CodeBlockKind, CodeBlockMetadata, MarkdownEvent, MarkdownTag, MarkdownTagEnd,
    ParsedMetadataBlock, parse_links_only, parse_markdown_with_options,
};
use crate::selection::Selection;
use crate::style::MarkdownStyle;

/// Per-diagram view state, keyed by source offset in [`Markdown::mermaid_views`].
struct MermaidViewState {
    /// Whether the source code is shown instead of the rendered diagram.
    showing_code: bool,
    /// The display scale relative to the diagram's natural size; 1.0 is 1:1.
    zoom: f32,
    /// Whether the user zoomed out to the fit-to-width floor. While set, the
    /// zoom tracks the container width so the diagram stays fully visible
    /// when the container is resized, instead of keeping a stale absolute
    /// zoom computed against the old width.
    zoomed_to_fit: bool,
    /// Horizontal scroll position, used when the diagram overflows.
    scroll_handle: ScrollHandle,
    /// The pending debounced re-raster scheduled by the last zoom change.
    debounce_task: Option<Task<()>>,
    /// Overrides the scroll container width, which tests can't obtain from
    /// the scroll handle since its bounds are only set during layout.
    #[cfg(test)]
    container_width_for_test: Option<Pixels>,
}

impl MermaidViewState {
    /// The width of the diagram's scroll container as of the last layout,
    /// if it has been laid out.
    fn container_width(&self) -> Option<Pixels> {
        #[cfg(test)]
        if let Some(width) = self.container_width_for_test {
            return Some(width);
        }
        Some(self.scroll_handle.bounds().size.width).filter(|width| *width > px(0.))
    }
}

impl Default for MermaidViewState {
    fn default() -> Self {
        Self {
            showing_code: false,
            zoom: 1.0,
            zoomed_to_fit: false,
            scroll_handle: ScrollHandle::new(),
            debounce_task: None,
            #[cfg(test)]
            container_width_for_test: None,
        }
    }
}

pub struct Markdown {
    source: SharedString,
    pub(super) selection: Selection,
    pub(super) pressed_link: Option<RenderedLink>,
    pub(super) pressed_footnote_ref: Option<RenderedFootnoteRef>,
    pub(super) autoscroll_request: Option<usize>,
    pending_heading_scroll: Option<SharedString>,
    pending_autoscroll: Option<usize>,
    pub(super) active_root_block: Option<usize>,
    pub(super) parsed_markdown: ParsedMarkdown,
    pub(super) images_by_source_offset: HashMap<usize, Arc<Image>>,
    should_reparse: bool,
    pending_parse: Option<Task<()>>,
    pub(super) focus_handle: FocusHandle,
    language_registry: Option<Arc<LanguageRegistry>>,
    fallback_code_block_language: Option<LanguageName>,
    pub(super) options: MarkdownOptions,
    pub(super) mermaid_state: MermaidState,
    _mermaid_theme_subscription: Option<Subscription>,
    /// Per-diagram view state (current tab, zoom, scroll position, and pending
    /// debounced re-raster) keyed by source offset. Distinct from
    /// [`MermaidState`], which caches the rendered diagrams themselves keyed by
    /// contents. All entries are retained against the parsed diagrams on each
    /// reparse, so a single map keeps that bookkeeping in one place.
    mermaid_views: HashMap<usize, MermaidViewState>,
    pub(super) copied_code_blocks: HashSet<ElementId>,
    wrapped_code_blocks: HashSet<usize>,
    code_block_scroll_handles: BTreeMap<usize, ScrollHandle>,
    context_menu_link: Option<SharedString>,
    context_menu_selected_text: Option<SharedString>,
    context_menu_selected_markdown: Option<SharedString>,
    pub(super) search_highlights: Rc<[Range<usize>]>,
    pub(super) active_search_highlight: Option<usize>,
}

#[derive(Clone, Copy, Default)]
pub struct MarkdownOptions {
    pub parse_links_only: bool,
    pub parse_html: bool,
    pub render_mermaid_diagrams: bool,
    pub parse_heading_slugs: bool,
    pub render_metadata_blocks: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CopyButtonVisibility {
    Hidden,
    AlwaysVisible,
    VisibleOnHover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapButtonVisibility {
    Hidden,
    AlwaysVisible,
    VisibleOnHover,
}

pub enum CodeBlockRenderer {
    Default {
        copy_button_visibility: CopyButtonVisibility,
        wrap_button_visibility: WrapButtonVisibility,
        border: bool,
    },
    Custom {
        render: CodeBlockRenderFn,
        /// A function that can modify the parent container after the code block
        /// content has been appended as a child element.
        transform: Option<CodeBlockTransformFn>,
    },
}

pub type CodeBlockRenderFn = Arc<
    dyn Fn(
        &CodeBlockKind,
        &ParsedMarkdown,
        Range<usize>,
        CodeBlockMetadata,
        &mut Window,
        &App,
    ) -> Div,
>;

pub type CodeBlockTransformFn =
    Arc<dyn Fn(AnyDiv, Range<usize>, CodeBlockMetadata, &mut Window, &App) -> AnyDiv>;

actions!(
    markdown,
    [
        /// Copies the selected text to the clipboard.
        Copy,
        /// Copies the selected text as markdown to the clipboard.
        CopyAsMarkdown
    ]
);

impl Markdown {
    pub fn new(
        source: SharedString,
        language_registry: Option<Arc<LanguageRegistry>>,
        fallback_code_block_language: Option<LanguageName>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_options(
            source,
            language_registry,
            fallback_code_block_language,
            MarkdownOptions::default(),
            cx,
        )
    }

    pub fn new_with_options(
        source: SharedString,
        language_registry: Option<Arc<LanguageRegistry>>,
        fallback_code_block_language: Option<LanguageName>,
        options: MarkdownOptions,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let theme_subscription = if options.render_mermaid_diagrams {
            Some(
                cx.observe_global::<theme::GlobalTheme>(|this: &mut Self, cx| {
                    this.invalidate_mermaid_cache(cx);
                }),
            )
        } else {
            None
        };
        let mut this = Self {
            source,
            selection: Selection::default(),
            pressed_link: None,
            pressed_footnote_ref: None,
            autoscroll_request: None,
            pending_heading_scroll: None,
            pending_autoscroll: None,
            active_root_block: None,
            should_reparse: false,
            images_by_source_offset: Default::default(),
            parsed_markdown: ParsedMarkdown::default(),
            pending_parse: None,
            focus_handle,
            language_registry,
            fallback_code_block_language,
            options,
            mermaid_state: MermaidState::default(),
            _mermaid_theme_subscription: theme_subscription,
            mermaid_views: HashMap::default(),
            copied_code_blocks: HashSet::default(),
            wrapped_code_blocks: HashSet::default(),
            code_block_scroll_handles: BTreeMap::default(),
            context_menu_link: None,
            context_menu_selected_text: None,
            context_menu_selected_markdown: None,
            search_highlights: Rc::default(),
            active_search_highlight: None,
        };
        this.parse(cx);
        this
    }

    pub fn new_text(source: SharedString, cx: &mut Context<Self>) -> Self {
        Self::new_with_options(
            source,
            None,
            None,
            MarkdownOptions {
                parse_links_only: true,
                ..Default::default()
            },
            cx,
        )
    }

    pub(super) fn is_code_block_wrapped(&self, id: usize) -> bool {
        self.wrapped_code_blocks.contains(&id)
    }

    pub(super) fn toggle_code_block_wrap(&mut self, id: usize) {
        if !self.wrapped_code_blocks.remove(&id) {
            self.wrapped_code_blocks.insert(id);
        }
    }

    pub(super) fn code_block_scroll_handle(&mut self, id: usize) -> Option<ScrollHandle> {
        (!self.is_code_block_wrapped(id)).then(|| {
            self.code_block_scroll_handles
                .entry(id)
                .or_insert_with(ScrollHandle::new)
                .clone()
        })
    }

    pub(super) fn retain_code_block_scroll_handles(&mut self, ids: &HashSet<usize>) {
        self.code_block_scroll_handles
            .retain(|id, _| ids.contains(id));
    }

    pub fn invalidate_mermaid_cache(&mut self, cx: &mut Context<Self>) {
        if !self.options.render_mermaid_diagrams || self.parsed_markdown.mermaid_diagrams.is_empty()
        {
            return;
        }

        self.mermaid_state.clear(cx);
        let mermaid_views = &self.mermaid_views;
        self.mermaid_state.update(
            &self.parsed_markdown,
            |source_offset| {
                mermaid_views
                    .get(&source_offset)
                    .map_or(1.0, |view| view.zoom)
            },
            cx,
        );
        cx.notify();
    }

    pub(crate) fn is_mermaid_showing_code(&self, source_offset: usize) -> bool {
        self.mermaid_views
            .get(&source_offset)
            .is_some_and(|view| view.showing_code)
    }

    pub(crate) fn toggle_mermaid_tab(&mut self, source_offset: usize) {
        let view = self.mermaid_views.entry(source_offset).or_default();
        view.showing_code = !view.showing_code;
    }

    pub(crate) fn mermaid_zoom_level(&self, source_offset: usize) -> f32 {
        self.mermaid_views
            .get(&source_offset)
            .map_or(1.0, |view| view.zoom)
    }

    /// The smallest zoom level for a diagram: the scale that makes it span
    /// the content width, capped at 1.0 so diagrams that already fit are
    /// never zoomed out below their natural size. Falls back to 1.0 when the
    /// diagram has no raster yet or the container hasn't been laid out.
    fn mermaid_min_zoom_level(&self, source_offset: usize) -> f32 {
        let Some(diagram) = self.parsed_markdown.mermaid_diagrams.get(&source_offset) else {
            return 1.0;
        };
        let Some(natural_size) = self.mermaid_state.natural_size(&diagram.contents) else {
            return 1.0;
        };
        let Some(container_width) = self
            .mermaid_views
            .get(&source_offset)
            .and_then(|view| view.container_width())
        else {
            return 1.0;
        };
        if natural_size.width <= container_width {
            return 1.0;
        }
        container_width / natural_size.width
    }

    pub(crate) fn set_mermaid_zoom_level(
        &mut self,
        source_offset: usize,
        zoom: f32,
        cx: &mut Context<Self>,
    ) {
        let min_zoom = self.mermaid_min_zoom_level(source_offset);
        let requested_zoom = zoom;
        let mut zoom = zoom.clamp(min_zoom, MERMAID_MAX_ZOOM);
        if (zoom - 1.0).abs() <= MERMAID_ZOOM_SNAP_TOLERANCE {
            zoom = 1.0;
        }
        // The user zoomed out to (or past) the fit-to-width floor. From here
        // on the zoom tracks the container width (see
        // `effective_mermaid_zoom_level`), until the user zooms back in. A
        // zoom landing exactly at 1.0 only sticks when it was clamped, so
        // resetting to the natural size never turns tracking on.
        let zoomed_to_fit = requested_zoom < min_zoom || (zoom <= min_zoom && zoom < 1.0);

        let debounce_task = self.schedule_mermaid_rerasterize(source_offset, cx);
        let view = self.mermaid_views.entry(source_offset).or_default();
        view.zoom = zoom;
        view.zoomed_to_fit = zoomed_to_fit;
        view.debounce_task = Some(debounce_task);
        cx.notify();
    }

    /// The zoom level to display a diagram at, syncing a fit-to-width zoom
    /// with the current container width. Called at render time so that a
    /// fully zoomed-out diagram stays stuck to the container width when the
    /// container is resized, rather than keeping a stale absolute zoom.
    pub(crate) fn effective_mermaid_zoom_level(
        &mut self,
        source_offset: usize,
        cx: &mut Context<Self>,
    ) -> f32 {
        let zoom = self.mermaid_zoom_level(source_offset);
        let zoomed_to_fit = self
            .mermaid_views
            .get(&source_offset)
            .is_some_and(|view| view.zoomed_to_fit);
        if !zoomed_to_fit {
            return zoom;
        }
        let min_zoom = self.mermaid_min_zoom_level(source_offset);
        if (min_zoom - zoom).abs() < 0.001 {
            return zoom;
        }
        let debounce_task = self.schedule_mermaid_rerasterize(source_offset, cx);
        if let Some(view) = self.mermaid_views.get_mut(&source_offset) {
            view.zoom = min_zoom;
            view.debounce_task = Some(debounce_task);
        }
        min_zoom
    }

    /// Schedules a debounced re-raster of a diagram at its current zoom.
    /// Storing the returned task in `MermaidViewState::debounce_task`
    /// replaces (and thereby cancels) the previous timer, debouncing the
    /// expensive re-raster until zoom changes settle. Until then, the
    /// existing raster is displayed scaled to the new zoom.
    fn schedule_mermaid_rerasterize(
        &self,
        source_offset: usize,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MERMAID_ZOOM_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.mermaid_views.get_mut(&source_offset) {
                    view.debounce_task = None;
                }
                this.rerasterize_mermaid_diagram(source_offset, cx);
            })
            .ok();
        })
    }

    pub(crate) fn mermaid_scroll_handle(&mut self, source_offset: usize) -> ScrollHandle {
        self.mermaid_views
            .entry(source_offset)
            .or_default()
            .scroll_handle
            .clone()
    }

    /// Re-rasterizes a single mermaid diagram at exactly the scale it is
    /// displayed at, reusing the cached parsed SVG so that neither mermaid
    /// layout nor SVG parsing is re-run. While the new raster is pending, the
    /// previous image keeps being displayed.
    fn rerasterize_mermaid_diagram(&mut self, source_offset: usize, cx: &mut Context<Self>) {
        let Some(diagram) = self.parsed_markdown.mermaid_diagrams.get(&source_offset) else {
            return;
        };
        let contents = diagram.contents.clone();
        let zoom = self.mermaid_zoom_level(source_offset);
        self.mermaid_state.rerasterize_diagram(&contents, zoom, cx);
        cx.notify();
    }

    pub(super) fn clear_code_block_scroll_handles(&mut self) {
        self.code_block_scroll_handles.clear();
    }

    pub(super) fn autoscroll_code_block(
        &self,
        source_index: usize,
        cursor_position: Point<Pixels>,
    ) {
        let Some((_, scroll_handle)) = self
            .code_block_scroll_handles
            .range(..=source_index)
            .next_back()
        else {
            return;
        };

        let bounds = scroll_handle.bounds();
        if cursor_position.y < bounds.top() || cursor_position.y > bounds.bottom() {
            return;
        }

        let horizontal_delta = if cursor_position.x < bounds.left() {
            bounds.left() - cursor_position.x
        } else if cursor_position.x > bounds.right() {
            bounds.right() - cursor_position.x
        } else {
            return;
        };

        let offset = scroll_handle.offset();
        scroll_handle.set_offset(point(offset.x + horizontal_delta, offset.y));
    }

    pub fn is_parsing(&self) -> bool { self.pending_parse.is_some() }

    pub fn scroll_to_heading_when_parsed(&mut self, slug: SharedString, cx: &mut Context<Self>) {
        if self.pending_parse.is_some() || self.source.is_empty() {
            self.pending_heading_scroll = Some(slug);
        } else {
            self.scroll_to_heading(&slug, cx);
        }
    }

    pub fn scroll_to_heading(&mut self, slug: &str, cx: &mut Context<Self>) -> Option<usize> {
        if let Some(source_index) = self.parsed_markdown.heading_slugs.get(slug).copied() {
            self.autoscroll_request = Some(source_index);
            cx.notify();
            Some(source_index)
        } else {
            None
        }
    }

    pub fn source(&self) -> &SharedString { &self.source }

    pub fn non_rendered_source_ranges(&self) -> Vec<Range<usize>> {
        if self.source != self.parsed_markdown.source {
            return Vec::new();
        }

        self.parsed_markdown.non_rendered_source_ranges()
    }

    pub fn first_code_block_language(&self) -> Option<Arc<Language>> {
        self.parsed_markdown.events.iter().find_map(|(_, event)| {
            let MarkdownEvent::Start(MarkdownTag::CodeBlock { kind, .. }) = event else {
                return None;
            };

            self.parsed_markdown.code_block_language(kind)
        })
    }

    pub fn append(&mut self, text: &str, cx: &mut Context<Self>) {
        self.source = SharedString::new(self.source.to_string() + text);
        self.parse(cx);
    }

    pub fn replace(&mut self, source: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.source = source.into();
        self.parse(cx);
    }

    pub fn request_autoscroll_to_source_index(
        &mut self,
        source_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.pending_parse.is_some() {
            self.pending_autoscroll = Some(source_index);
        } else {
            self.autoscroll_request = Some(source_index);
        }
        cx.refresh_windows();
    }

    pub(super) fn footnote_definition_content_start(&self, label: &SharedString) -> Option<usize> {
        self.parsed_markdown
            .footnote_definitions
            .get(label)
            .copied()
    }

    pub fn set_active_root_for_source_index(
        &mut self,
        source_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let active_root_block =
            source_index.and_then(|index| self.parsed_markdown.root_block_for_source_index(index));
        if self.active_root_block == active_root_block {
            return;
        }

        self.active_root_block = active_root_block;
        cx.notify();
    }

    pub fn reset(&mut self, source: SharedString, cx: &mut Context<Self>) {
        if &source == self.source() {
            if self.pending_parse.is_none() {
                if let Some(slug) = self.pending_heading_scroll.take() {
                    self.pending_autoscroll = None;
                    self.scroll_to_heading(&slug, cx);
                } else if let Some(source_index) = self.pending_autoscroll.take() {
                    self.autoscroll_request = Some(source_index);
                    cx.refresh_windows();
                }
            }
            return;
        }
        if !self.source.is_empty() {
            self.pending_heading_scroll = None;
        }
        self.source = source;
        self.selection = Selection::default();
        self.autoscroll_request = None;
        self.pending_autoscroll = None;
        self.pending_parse = None;
        self.should_reparse = false;
        self.search_highlights = Rc::default();
        self.active_search_highlight = None;
        // Don't clear parsed_markdown here - keep existing content visible until new parse completes
        self.parse(cx);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn parsed_markdown(&self) -> &ParsedMarkdown { &self.parsed_markdown }

    pub fn escape(s: &str) -> Cow<'_, str> { crate::escape::escape(s) }

    pub fn has_selection(&self) -> bool { self.selection.end > self.selection.start }

    pub fn selected_source(&self) -> Option<&str> {
        if self.selection.end <= self.selection.start {
            return None;
        }
        self.source.get(self.selection.start..self.selection.end)
    }

    pub fn set_search_highlights(
        &mut self,
        highlights: Vec<Range<usize>>,
        active: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        debug_assert!(
            highlights
                .windows(2)
                .all(|ranges| (ranges[0].start, ranges[0].end) <= (ranges[1].start, ranges[1].end))
        );
        self.search_highlights = highlights.into();
        self.active_search_highlight =
            active.filter(|active| *active < self.search_highlights.len());
        cx.notify();
    }

    pub fn clear_search_highlights(&mut self, cx: &mut Context<Self>) {
        if !self.search_highlights.is_empty() || self.active_search_highlight.is_some() {
            self.search_highlights = Rc::default();
            self.active_search_highlight = None;
            cx.notify();
        }
    }

    pub fn set_active_search_highlight(&mut self, active: Option<usize>, cx: &mut Context<Self>) {
        let active = active.filter(|active| *active < self.search_highlights.len());
        if self.active_search_highlight != active {
            self.active_search_highlight = active;
            cx.notify();
        }
    }

    pub fn search_highlights(&self) -> &[Range<usize>] { &self.search_highlights }

    pub fn active_search_highlight(&self) -> Option<usize> { self.active_search_highlight }

    pub(super) fn copy(&self, text: &RenderedText, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection.end <= self.selection.start {
            return;
        }
        let text = text.text_for_range(self.selection.start..self.selection.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub(super) fn copy_as_markdown(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.context_menu_selected_markdown.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            return;
        }
        if self.selection.end <= self.selection.start {
            return;
        }
        let text = self
            .parsed_markdown
            .rebalanced_markdown_for_selection(self.selection.start..self.selection.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub(super) fn capture_for_context_menu(
        &mut self,
        link: Option<SharedString>,
        rendered_text: Option<&RenderedText>,
    ) {
        let range = self.selection.start..self.selection.end;
        if range.end > range.start {
            self.context_menu_selected_markdown = Some(SharedString::new(
                self.parsed_markdown
                    .rebalanced_markdown_for_selection(range.clone()),
            ));
            self.context_menu_selected_text = rendered_text
                .map(|text| text.text_for_range(range))
                .map(SharedString::new)
                .or_else(|| self.context_menu_selected_markdown.clone());
        } else {
            self.context_menu_selected_markdown = None;
            self.context_menu_selected_text = None;
        }
        self.context_menu_link = link;
    }

    /// Returns the URL of the link that was most recently right-clicked, if any.
    /// This is set during a right-click mouse-down event and can be read by parent
    /// views to include a "Copy Link" item in their context menus.
    pub fn context_menu_link(&self) -> Option<&SharedString> { self.context_menu_link.as_ref() }

    /// Returns the rendered (plain) text that was selected when the most recent
    /// context menu invocation happened.
    pub fn context_menu_selected_text(&self) -> Option<&SharedString> {
        self.context_menu_selected_text.as_ref()
    }

    /// Returns the markdown that was selected when the most recent context
    /// menu invocation happened, rebalanced via
    /// [`ParsedMarkdown::rebalanced_markdown_for_selection`].
    pub fn context_menu_selected_markdown(&self) -> Option<&SharedString> {
        self.context_menu_selected_markdown.as_ref()
    }

    fn parse(&mut self, cx: &mut Context<Self>) {
        if self.source.is_empty() {
            self.should_reparse = false;
            self.pending_parse.take();
            self.pending_heading_scroll = None;
            self.pending_autoscroll = None;
            self.parsed_markdown = ParsedMarkdown {
                source: self.source.clone(),
                ..Default::default()
            };
            self.active_root_block = None;
            self.images_by_source_offset.clear();
            self.mermaid_state.clear(cx);
            cx.notify();
            cx.refresh_windows();
            return;
        }

        if self.pending_parse.is_some() {
            self.should_reparse = true;
            return;
        }
        self.should_reparse = false;
        self.pending_parse = Some(self.start_background_parse(cx));
    }

    fn start_background_parse(&self, cx: &Context<Self>) -> Task<()> {
        let source = self.source.clone();
        let should_parse_links_only = self.options.parse_links_only;
        let should_parse_html = self.options.parse_html;
        let should_render_mermaid_diagrams = self.options.render_mermaid_diagrams;
        let should_parse_heading_slugs = self.options.parse_heading_slugs;
        let should_parse_metadata_blocks = self.options.render_metadata_blocks;
        let language_registry = self.language_registry.clone();
        let fallback = self.fallback_code_block_language.clone();

        let parsed = cx.background_spawn(async move {
            if should_parse_links_only {
                return (
                    ParsedMarkdown {
                        events: Arc::from(parse_links_only(source.as_ref())),
                        source,
                        languages_by_name: TreeMap::default(),
                        languages_by_path: TreeMap::default(),
                        root_block_starts: Arc::default(),
                        html_blocks: BTreeMap::default(),
                        metadata_blocks: BTreeMap::default(),
                        mermaid_diagrams: BTreeMap::default(),
                        heading_slugs: HashMap::default(),
                        footnote_definitions: HashMap::default(),
                        link_definition_spans: Arc::default(),
                        fallback_code_block_language: None,
                        code_block_highlights: Arc::default(),
                    },
                    Default::default(),
                );
            }

            let parsed = parse_markdown_with_options(
                &source,
                should_parse_html,
                should_parse_heading_slugs,
                should_parse_metadata_blocks,
            );
            let events = parsed.events;
            let language_names = parsed.language_names;
            let paths = parsed.language_paths;
            let root_block_starts = parsed.root_block_starts;
            let html_blocks = parsed.html_blocks;
            let metadata_blocks = parsed.metadata_blocks;
            let heading_slugs = parsed.heading_slugs;
            let footnote_definitions = parsed.footnote_definitions;
            let link_definition_spans = parsed.link_definition_spans;
            let has_untagged_code_block = parsed.has_untagged_code_block;
            let mermaid_diagrams = if should_render_mermaid_diagrams {
                extract_mermaid_diagrams(&source, &events)
            } else {
                BTreeMap::default()
            };
            let mut images_by_source_offset = HashMap::default();
            let mut languages_by_name = TreeMap::default();
            let mut languages_by_path = TreeMap::default();
            let mut fallback_code_block_language = None;
            if let Some(registry) = language_registry.as_ref() {
                for name in language_names {
                    if let Ok(language) = registry.language_for_name_or_extension(&name).await {
                        languages_by_name.insert(name, language);
                    }
                }

                for path in paths {
                    if let Ok(language) = registry
                        .load_language_for_file_path(Path::new(path.as_ref()))
                        .await
                    {
                        languages_by_path.insert(path, language);
                    }
                }

                if has_untagged_code_block && let Some(fallback) = &fallback {
                    fallback_code_block_language =
                        registry.language_for_name(fallback.as_ref()).await.ok();
                }
            }

            for (range, event) in &events {
                if let MarkdownEvent::Start(MarkdownTag::Image { dest_url, .. }) = event
                    && let Some(data_url) = dest_url.strip_prefix("data:")
                {
                    let Some((mime_info, data)) = data_url.split_once(',') else {
                        continue;
                    };
                    let Some((mime_type, encoding)) = mime_info.split_once(';') else {
                        continue;
                    };
                    let Some(format) = ImageFormat::from_mime_type(mime_type) else {
                        continue;
                    };
                    let is_base64 = encoding == "base64";
                    if is_base64
                        && let Some(bytes) = base64::prelude::BASE64_STANDARD
                            .decode(data)
                            .log_with_level(Level::Debug)
                    {
                        let image = Arc::new(Image::from_bytes(format, bytes));
                        images_by_source_offset.insert(range.start, image);
                    }
                }
            }

            let mut parsed = ParsedMarkdown {
                source,
                events: Arc::from(events),
                languages_by_name,
                languages_by_path,
                root_block_starts: Arc::from(root_block_starts),
                html_blocks,
                metadata_blocks,
                mermaid_diagrams,
                heading_slugs,
                footnote_definitions,
                link_definition_spans: Arc::from(link_definition_spans),
                fallback_code_block_language,
                code_block_highlights: Arc::default(),
            };
            parsed.code_block_highlights = Arc::new(compute_code_block_highlights(&parsed));
            (parsed, images_by_source_offset)
        });

        cx.spawn(async move |this, cx| {
            let (parsed, images_by_source_offset) = parsed.await;

            this.update(cx, |this, cx| {
                this.parsed_markdown = parsed;
                this.images_by_source_offset = images_by_source_offset;
                if this.active_root_block.is_some_and(|block_index| {
                    block_index >= this.parsed_markdown.root_block_starts.len()
                }) {
                    this.active_root_block = None;
                }
                if this.options.render_mermaid_diagrams {
                    let parsed_markdown = this.parsed_markdown.clone();
                    this.mermaid_views
                        .retain(|offset, _| parsed_markdown.mermaid_diagrams.contains_key(offset));
                    let mermaid_views = &this.mermaid_views;
                    this.mermaid_state.update(
                        &parsed_markdown,
                        |source_offset| {
                            mermaid_views
                                .get(&source_offset)
                                .map_or(1.0, |view| view.zoom)
                        },
                        cx,
                    );
                } else {
                    this.mermaid_state.clear(cx);
                    this.mermaid_views.clear();
                }
                this.pending_parse.take();
                if this.should_reparse {
                    this.parse(cx);
                } else if let Some(slug) = this.pending_heading_scroll.take()
                    && let Some(source_index) =
                        this.parsed_markdown.heading_slugs.get(&slug).copied()
                {
                    this.pending_autoscroll = None;
                    this.autoscroll_request = Some(source_index);
                } else if let Some(source_index) = this.pending_autoscroll.take() {
                    this.autoscroll_request = Some(source_index);
                }
                cx.notify();
                cx.refresh_windows();
            })
            .ok();
        })
    }
}

impl Focusable for Markdown {
    fn focus_handle(&self, _cx: &App) -> FocusHandle { self.focus_handle.clone() }
}
