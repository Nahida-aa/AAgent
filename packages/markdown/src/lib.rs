//! Markdown 解析与渲染。
//!
//! 结构对齐 zed `crates/markdown/src/markdown.rs`（单文件），这里按职责拆成
//! 子模块；拆分的副作用是父模块要负责把「zed 里同文件可见」的东西集中导入
//! 与再导出（类型别名、pulldown 类型等）。

mod builder;
mod element;
mod entity;
mod escape;
mod highlights;
pub mod html;
mod mermaid;
mod parsed;
pub mod parser;
mod path_range;
mod rendered;
mod selection;
mod style;

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ClipboardItem, CursorStyle, DispatchPhase, Edges, Entity,
    FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId, Hitbox, Hsla, Image,
    ImageFormat, ImageSource, InputHandler, KeyContext, Length, MouseButton, MouseDownEvent,
    MouseEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle, SharedString, Stateful,
    StrikethroughStyle, StyleRefinement, StyledImage, StyledText, Subscription, Task, TextAlign,
    TextLayout, TextRun, TextStyle, TextStyleRefinement, UTF16Selection, Window,
    WrappedLineLayout, actions, canvas, img, point, px, quad, relative, size,
};
use language::{
    Bias, CharClassifier, Language, LanguageRegistry, OffsetUtf16, ResolvedHighlights, Rope,
};
use pulldown_cmark::BlockQuoteKind;

pub(crate) use builder::MarkdownElementBuilder;
pub use element::{AutoscrollBehavior, MarkdownElement};
pub use entity::{
    CodeBlockRenderFn, CodeBlockRenderer, CodeBlockTransformFn, CopyButtonVisibility, Markdown,
    MarkdownOptions, WrapButtonVisibility,CopyAsMarkdown,Copy
};
pub use escape::MarkdownEscaper;
pub use parsed::ParsedMarkdown;
pub use path_range::{LineCol, PathWithRange};
pub use style::{BlockQuoteKindColors, HeadingLevelStyles, MarkdownFont, MarkdownStyle};

/// mermaid 缩放上限。
pub(crate) const MERMAID_MAX_ZOOM: f32 = 2.0;
/// 与 1.0 的距离在此容差内会吸附回 1.0，方便用户回到默认大小。
pub(crate) const MERMAID_ZOOM_SNAP_TOLERANCE: f32 = 0.05;
pub(crate) const MERMAID_ZOOM_DEBOUNCE: std::time::Duration =
    std::time::Duration::from_millis(300);

/// 按目标 URL 自定义链接样式；返回 `None` 用默认样式。
pub type LinkStyleCallback = Rc<dyn Fn(&str, &App) -> Option<TextStyleRefinement>>;
pub type CodeSpanLinkCallback = Arc<dyn Fn(&str, &App) -> Option<SharedString> + 'static>;
pub type UrlHoverCallback = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;
pub type SourceClickCallback = Box<dyn Fn(usize, usize, &mut Window, &mut App) -> bool>;
pub type CheckboxToggleCallback = Rc<dyn Fn(Range<usize>, bool, &mut Window, &mut App)>;
/// mermaid 图缩放级别变化时回调，便于滚动容器保持锚定。
pub type MermaidZoomCallback = Rc<dyn Fn(&mut Window, &mut App)>;

struct MarkdownInputHandler {
    markdown: Entity<Markdown>,
    rendered_text: crate::rendered::RenderedText,
}

impl MarkdownInputHandler {
    fn new(markdown: Entity<Markdown>, rendered_text: crate::rendered::RenderedText) -> Self {
        Self {
            markdown,
            rendered_text,
        }
    }
}

impl InputHandler for MarkdownInputHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.markdown.update(cx, |markdown, _cx| {
            let range = markdown.selection.start..markdown.selection.end;
            Some(UTF16Selection {
                range: self.rendered_text.utf16_range_for_source_range(range),
                reversed: markdown.selection.reversed,
            })
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        if range_utf16.start > range_utf16.end {
            return None;
        }

        Some(
            self.rendered_text
                .text_for_utf16_range(range_utf16, adjusted_range),
        )
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let source_range = self.rendered_text.source_range_for_utf16_range(range_utf16);
        self.rendered_text
            .bounds_for_source_range(source_range.clone())
            .into_iter()
            .next()
            .or_else(|| {
                self.rendered_text
                    .position_for_source_index(source_range.start)
                    .map(|(position, line_height)| Bounds {
                        origin: position,
                        size: size(px(0.), line_height),
                    })
            })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        let source_index = self
            .rendered_text
            .source_index_for_visible_position(point)?;
        Some(
            self.rendered_text
                .utf16_index_for_source_index(source_index),
        )
    }

    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        false
    }
}
