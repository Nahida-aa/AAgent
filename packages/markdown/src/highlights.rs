use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, Pixels, Style, Window,
};
use smallvec::SmallVec;

use crate::rendered::RenderedLine;

pub(crate) struct MarkdownHighlights {
    /// Search highlights, sorted by range start.
    pub(crate) search_highlights: Rc<[Range<usize>]>,
    pub(crate) active_search_highlight: Option<usize>,
    pub(crate) search_match_color: Hsla,
    pub(crate) active_search_match_color: Hsla,
    pub(crate) selection: Option<(Range<usize>, Hsla)>,
    /// Index of the first search highlight that may intersect the next line.
    pub(crate) next_search_highlight_ix: usize,
}

impl MarkdownHighlights {
    /// Returns the highlighted ranges intersecting the given source range,
    /// clamped to it, in paint order.
    pub(crate) fn highlights_for_line(
        &mut self,
        source_range: Range<usize>,
    ) -> SmallVec<[(Range<usize>, Hsla); 1]> {
        let mut highlights = SmallVec::new();

        self.next_search_highlight_ix += self.search_highlights[self.next_search_highlight_ix..]
            .iter()
            .take_while(|range| range.end <= source_range.start)
            .count();

        for (ix, range) in self
            .search_highlights
            .iter()
            .enumerate()
            .skip(self.next_search_highlight_ix)
        {
            if range.start >= source_range.end {
                break;
            }
            let clamped = range.start.max(source_range.start)..range.end.min(source_range.end);
            if clamped.start < clamped.end {
                let color = if Some(ix) == self.active_search_highlight {
                    self.active_search_match_color
                } else {
                    self.search_match_color
                };
                highlights.push((clamped, color));
            }
        }
        if let Some((range, color)) = &self.selection {
            let clamped = range.start.max(source_range.start)..range.end.min(source_range.end);
            if clamped.start < clamped.end {
                highlights.push((clamped, *color));
            }
        }
        highlights
    }
}

/// Wraps a rendered line's text and paints the line's highlight quads during the
/// line's own paint, so the ancestor content masks clip them like they clip the
/// glyphs themselves.
pub(crate) struct HighlightedLine {
    pub(crate) text: AnyElement,
    pub(crate) line: Rc<RenderedLine>,
}

impl Element for HighlightedLine {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> { None }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        (self.text.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.line.paint_code_chips(window);
        self.text.paint(window, cx);
        self.line.paint_highlights(window);
    }
}

impl IntoElement for HighlightedLine {
    type Element = Self;

    fn into_element(self) -> Self::Element { self }
}
