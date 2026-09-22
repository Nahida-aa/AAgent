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
