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
    ImageFormat, ImageSource, KeyContext, Length, MouseButton, MouseDownEvent, MouseEvent,
    MouseMoveEvent, MouseUpEvent, Point, ScrollHandle, SharedString, Stateful, StrikethroughStyle,
    StyleRefinement, StyledImage, StyledText, Subscription, Task, TextAlign, TextLayout, TextRun,
    TextStyle, TextStyleRefinement, Window, WrappedLineLayout, actions, canvas, img, point, quad,
    relative, size,
};
use pulldown_cmark::BlockQuoteKind;

pub(crate) use builder::MarkdownElementBuilder;
pub use element::{AutoscrollBehavior, MarkdownElement};
pub use entity::{
    CodeBlockRenderFn, CodeBlockRenderer, CodeBlockTransformFn, CopyButtonVisibility, Markdown,
    MarkdownOptions, WrapButtonVisibility,
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
