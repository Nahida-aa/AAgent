mod block_element;
mod cell_style;
mod highlight;
mod input;
mod layout;
mod layout_grid;
mod mouse;
mod paint;
mod prepaint;
mod rem_size;

pub use block_element::BlockElementLayoutRect;
pub use cell_style::{convert_color, is_blank};
pub use layout::{BatchedTextRun, LayoutPoint, LayoutRect, LayoutState};

use std::rc::Rc;

use gpui::{
    App, Context, DispatchPhase, Element, ElementId, Entity, FocusHandle, GlobalElementId,
    InteractiveElement, Interactivity, IntoElement, Length, Pixels, StatefulInteractiveElement,
    WeakEntity, Window, relative,
};
use terminal::Terminal;
use workspace::Workspace;

use crate::{BlockProperties, ContentMode, TerminalMode, TerminalView};

/// The GPUI element that paints the terminal.
/// We need to keep a reference to the model for mouse events, do we need it for any other terminal stuff, or can we move that to connection?
pub struct TerminalElement {
    terminal: Entity<Terminal>,
    terminal_view: Entity<TerminalView>,
    workspace: WeakEntity<Workspace>,
    focus: FocusHandle,
    focused: bool,
    cursor_visible: bool,
    interactivity: Interactivity,
    mode: TerminalMode,
    block_below_cursor: Option<Rc<BlockProperties>>,
}

impl InteractiveElement for TerminalElement {
    fn interactivity(&mut self) -> &mut Interactivity { &mut self.interactivity }
}
impl StatefulInteractiveElement for TerminalElement {}

impl TerminalElement {
    pub fn new(
        terminal: Entity<Terminal>,
        terminal_view: Entity<TerminalView>,
        workspace: WeakEntity<Workspace>,
        focus: FocusHandle,
        focused: bool,
        cursor_visible: bool,
        block_below_cursor: Option<Rc<BlockProperties>>,
        mode: TerminalMode,
    ) -> TerminalElement {
        TerminalElement {
            terminal,
            terminal_view,
            workspace,
            focused,
            focus: focus.clone(),
            cursor_visible,
            block_below_cursor,
            mode,
            interactivity: Default::default(),
        }
        .track_focus(&focus)
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> { self.interactivity.element_id.clone() }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let height: Length = match self.terminal_view.read(cx).content_mode(window, cx) {
            ContentMode::Inline {
                displayed_lines,
                total_lines: _,
            } => {
                let rem_size = window.rem_size();
                let line_height = f32::from(window.text_style().font_size.to_pixels(rem_size))
                    * TerminalSettings::get_global(cx).line_height.value();
                // Round up to a whole device pixel to prevent pixel snapping from rounding down,
                // which would result in the terminal being one row short after flooring.
                let scale_factor = window.scale_factor().max(1.);
                let height = displayed_lines as f32 * line_height;
                px((height * scale_factor).ceil() / scale_factor).into()
            }
            ContentMode::Scrollable => {
                if let TerminalMode::Embedded { .. } = &self.mode {
                    let term = self.terminal.read(cx);
                    if !term.scrolled_to_top() && !term.scrolled_to_bottom() && self.focused {
                        self.interactivity.occlude_mouse();
                    }
                }

                relative(1.).into()
            }
        };

        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |mut style, window, cx| {
                style.size.width = relative(1.).into();
                style.size.height = height;

                window.request_layout(style, None, cx)
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        prepaint::prepaint(self, global_id, inspector_id, bounds, window, cx)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint::paint(self, global_id, inspector_id, bounds, layout, window, cx)
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;
    fn into_element(self) -> Self::Element { self }
}
