mod element;
pub mod html;
mod mermaid;
pub mod parser;
mod path_range;
mod selection;
mod style;

mod builder;
mod element;
mod entity;
mod escape;
mod highlights;
mod parsed;
mod rendered;
mod style;
use collections::{HashMap, HashSet};
pub use element::MarkdownElement;
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ClipboardItem, CursorStyle, DispatchPhase, Edges, Entity,
    FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId, Hitbox, Hsla, Image,
    ImageFormat, ImageSource, KeyContext, Length, MouseButton, MouseDownEvent, MouseEvent,
    MouseMoveEvent, MouseUpEvent, Point, ScrollHandle, SharedString, Stateful, StrikethroughStyle,
    StyleRefinement, StyledImage, StyledText, Subscription, Task, TextAlign, TextLayout, TextRun,
    TextStyle, TextStyleRefinement, WrappedLineLayout, actions, canvas, img, point, quad, relative,
    size,
};
pub use style::MarkdownStyle;

// 公开 API re-exports
pub use element::{AutoscrollBehavior, MarkdownElement};
pub use entity::{
    CodeBlockRenderFn, CodeBlockRenderer, CodeBlockTransformFn, CopyButtonVisibility, Markdown,
    MarkdownOptions, MermaidZoomCallback, WrapButtonVisibility,
};
pub use escape::MarkdownEscaper;
pub use parsed::ParsedMarkdown;
pub use path_range::{LineCol, PathWithRange};
pub use style::{BlockQuoteKindColors, HeadingLevelStyles, MarkdownFont, MarkdownStyle};

// 共享类型别名
use gpui::{App, SharedString, TextStyleRefinement, Window};
use std::sync::Arc;

pub type LinkStyleCallback = std::rc::Rc<dyn Fn(&str, &App) -> Option<TextStyleRefinement>>;
pub type CodeSpanLinkCallback = Arc<dyn Fn(&str, &App) -> Option<SharedString> + 'static>;
pub type UrlHoverCallback = std::rc::Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;
pub type SourceClickCallback = Box<dyn Fn(usize, usize, &mut Window, &mut App) -> bool>;
pub type CheckboxToggleCallback =
    std::rc::Rc<dyn Fn(std::ops::Range<usize>, bool, &mut Window, &mut App)>;

// --------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CopyButtonVisibility {
    Hidden,
    AlwaysVisible,
    VisibleOnHover,
}

pub struct Markdown {
    source: SharedString,
    selection: Selection,
    pressed_link: Option<RenderedLink>,
    pressed_footnote_ref: Option<RenderedFootnoteRef>,
    autoscroll_request: Option<usize>,
    pending_heading_scroll: Option<SharedString>,
    pending_autoscroll: Option<usize>,
    active_root_block: Option<usize>,
    parsed_markdown: ParsedMarkdown,
    images_by_source_offset: HashMap<usize, Arc<Image>>,
    should_reparse: bool,
    pending_parse: Option<Task<()>>,
    focus_handle: FocusHandle,
    language_registry: Option<Arc<LanguageRegistry>>,
    fallback_code_block_language: Option<LanguageName>,
    options: MarkdownOptions,
    mermaid_state: MermaidState,
    _mermaid_theme_subscription: Option<Subscription>,
    /// Per-diagram view state (current tab, zoom, scroll position, and pending
    /// debounced re-raster) keyed by source offset. Distinct from
    /// [`MermaidState`], which caches the rendered diagrams themselves keyed by
    /// contents. All entries are retained against the parsed diagrams on each
    /// reparse, so a single map keeps that bookkeeping in one place.
    mermaid_views: HashMap<usize, MermaidViewState>,
    copied_code_blocks: HashSet<ElementId>,
    wrapped_code_blocks: HashSet<usize>,
    code_block_scroll_handles: BTreeMap<usize, ScrollHandle>,
    context_menu_link: Option<SharedString>,
    context_menu_selected_text: Option<SharedString>,
    context_menu_selected_markdown: Option<SharedString>,
    search_highlights: Rc<[Range<usize>]>,
    active_search_highlight: Option<usize>,
}
