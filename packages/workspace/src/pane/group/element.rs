use std::cell::RefCell;
use std::iter;
use std::mem;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Along, AnyElement, App, Axis, BorderStyle, Bounds, CursorStyle, Element, GlobalElementId,
    Hitbox, HitboxBehavior, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Size, Style, WeakEntity, Window, px, relative, size,
};
use parking_lot::Mutex;
use settings::Settings;
use smallvec::SmallVec;
use ui::prelude::*;
use util::ResultExt;

use crate::Workspace;
use crate::WorkspaceSettings;

use super::{HANDLE_HITBOX_SIZE, HORIZONTAL_MIN_SIZE, VERTICAL_MIN_SIZE};

const DIVIDER_SIZE: f32 = 1.0;

pub(super) fn pane_axis(
    axis: Axis,
    basis: usize,
    flexes: Arc<Mutex<Vec<f32>>>,
    bounding_boxes: Arc<Mutex<Vec<Option<Bounds<Pixels>>>>>,
    workspace: WeakEntity<Workspace>,
) -> PaneAxisElement {
    PaneAxisElement {
        axis,
        basis,
        flexes,
        bounding_boxes,
        children: SmallVec::new(),
        active_pane_ix: None,
        workspace,
        is_leaf_pane_mask: Vec::new(),
    }
}
pub struct PaneAxisElement {
    axis: Axis,
    basis: usize,
    /// Equivalent to ColumnWidths (but in terms of flexes instead of percentages)
    /// For example, flexes "1.33, 1, 1", instead of "40%, 30%, 30%"
    flexes: Arc<Mutex<Vec<f32>>>,
    bounding_boxes: Arc<Mutex<Vec<Option<Bounds<Pixels>>>>>,
    children: SmallVec<[AnyElement; 2]>,
    active_pane_ix: Option<usize>,
    workspace: WeakEntity<Workspace>,
    // Track which children are leaf panes (Member::Pane) vs axes (Member::Axis)
    is_leaf_pane_mask: Vec<bool>,
}

pub struct PaneAxisLayout {
    dragged_handle: Rc<RefCell<Option<usize>>>,
    children: Vec<PaneAxisChildLayout>,
}

struct PaneAxisChildLayout {
    bounds: Bounds<Pixels>,
    element: AnyElement,
    handle: Option<PaneAxisHandleLayout>,
    is_leaf_pane: bool,
}

struct PaneAxisHandleLayout {
    hitbox: Hitbox,
    divider_bounds: Bounds<Pixels>,
}

impl PaneAxisElement {
    pub fn with_active_pane(mut self, active_pane_ix: Option<usize>) -> Self {
        self.active_pane_ix = active_pane_ix;
        self
    }

    pub fn with_is_leaf_pane_mask(mut self, mask: Vec<bool>) -> Self {
        self.is_leaf_pane_mask = mask;
        self
    }

    fn compute_resize(
        flexes: &Arc<Mutex<Vec<f32>>>,
        e: &MouseMoveEvent,
        ix: usize,
        axis: Axis,
        child_start: Point<Pixels>,
        container_size: Size<Pixels>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let min_size = match axis {
            Axis::Horizontal => px(HORIZONTAL_MIN_SIZE),
            Axis::Vertical => px(VERTICAL_MIN_SIZE),
        };
        let mut flexes = flexes.lock();
        debug_assert!(flex_values_in_bounds(flexes.as_slice()));

        // Math to convert a flex value to a pixel value
        let size = move |ix, flexes: &[f32]| {
            container_size.along(axis) * (flexes[ix] / flexes.len() as f32)
        };

        // Don't allow resizing to less than the minimum size, if elements are already too small
        if min_size - px(1.) > size(ix, flexes.as_slice()) {
            return;
        }

        // This is basically a "bucket" of pixel changes that need to be applied in response to this
        // mouse event. Probably a small, fractional number like 0.5 or 1.5 pixels
        let mut proposed_current_pixel_change =
            (e.position - child_start).along(axis) - size(ix, flexes.as_slice());

        // This takes a pixel change, and computes the flex changes that correspond to this pixel change
        // as well as the next one, for some reason
        let flex_changes = |pixel_dx, target_ix, next: isize, flexes: &[f32]| {
            let flex_change = pixel_dx / container_size.along(axis);
            let current_target_flex = flexes[target_ix] + flex_change;
            let next_target_flex = flexes[(target_ix as isize + next) as usize] - flex_change;
            (current_target_flex, next_target_flex)
        };

        // Generate the list of flex successors, from the current index.
        // If you're dragging column 3 forward, out of 6 columns, then this code will produce [4, 5, 6]
        // If you're dragging column 3 backward, out of 6 columns, then this code will produce [2, 1, 0]
        let mut successors = iter::from_fn({
            let forward = proposed_current_pixel_change > px(0.);
            let mut ix_offset = 0;
            let len = flexes.len();
            move || {
                let result = if forward {
                    (ix + 1 + ix_offset < len).then(|| ix + ix_offset)
                } else {
                    (ix as isize - ix_offset as isize >= 0).then(|| ix - ix_offset)
                };

                ix_offset += 1;

                result
            }
        });

        // Now actually loop over these, and empty our bucket of pixel changes
        while proposed_current_pixel_change.abs() > px(0.) {
            let Some(current_ix) = successors.next() else {
                break;
            };

            let next_target_size = Pixels::max(
                size(current_ix + 1, flexes.as_slice()) - proposed_current_pixel_change,
                min_size,
            );

            let current_target_size = Pixels::max(
                size(current_ix, flexes.as_slice()) + size(current_ix + 1, flexes.as_slice())
                    - next_target_size,
                min_size,
            );

            let current_pixel_change = current_target_size - size(current_ix, flexes.as_slice());

            let (current_target_flex, next_target_flex) =
                flex_changes(current_pixel_change, current_ix, 1, flexes.as_slice());

            flexes[current_ix] = current_target_flex;
            flexes[current_ix + 1] = next_target_flex;

            proposed_current_pixel_change -= current_pixel_change;
        }

        workspace
            .update(cx, |this, cx| this.serialize_workspace(window, cx))
            .log_err();
        cx.stop_propagation();
        window.refresh();
    }

    fn layout_handle(
        axis: Axis,
        pane_bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> PaneAxisHandleLayout {
        let handle_bounds = Bounds {
            origin: pane_bounds.origin.apply_along(axis, |origin| {
                origin + pane_bounds.size.along(axis) - px(HANDLE_HITBOX_SIZE / 2.)
            }),
            size: pane_bounds
                .size
                .apply_along(axis, |_| px(HANDLE_HITBOX_SIZE)),
        };
        let divider_bounds = Bounds {
            origin: pane_bounds
                .origin
                .apply_along(axis, |origin| origin + pane_bounds.size.along(axis)),
            size: pane_bounds.size.apply_along(axis, |_| px(DIVIDER_SIZE)),
        };

        PaneAxisHandleLayout {
            hitbox: window.insert_hitbox(handle_bounds, HitboxBehavior::BlockMouse),
            divider_bounds,
        }
    }
}

impl IntoElement for PaneAxisElement {
    type Element = Self;

    fn into_element(self) -> Self::Element { self }
}

impl Element for PaneAxisElement {
    type RequestLayoutState = ();
    type PrepaintState = PaneAxisLayout;

    fn id(&self) -> Option<ElementId> { Some(self.basis.into()) }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let style = Style {
            flex_grow: 1.,
            flex_shrink: 1.,
            flex_basis: relative(0.).into(),
            size: size(relative(1.).into(), relative(1.).into()),
            ..Style::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> PaneAxisLayout {
        let dragged_handle = window.with_element_state::<Rc<RefCell<Option<usize>>>, _>(
            global_id.unwrap(),
            |state, _cx| {
                let state = state.unwrap_or_else(|| Rc::new(RefCell::new(None)));
                (state.clone(), state)
            },
        );
        let flexes = self.flexes.lock().clone();
        let len = self.children.len();
        debug_assert!(flexes.len() == len);
        debug_assert!(flex_values_in_bounds(flexes.as_slice()));

        let total_flex = len as f32;

        let mut origin = bounds.origin;
        let space_per_flex = bounds.size.along(self.axis) / total_flex;

        let mut bounding_boxes = self.bounding_boxes.lock();
        bounding_boxes.clear();

        let mut layout = PaneAxisLayout {
            dragged_handle,
            children: Vec::new(),
        };
        for (ix, mut child) in mem::take(&mut self.children).into_iter().enumerate() {
            let child_flex = flexes[ix];

            let child_size = bounds
                .size
                .apply_along(self.axis, |_| space_per_flex * child_flex)
                .map(|d| d.round());

            let child_bounds = Bounds {
                origin,
                size: child_size,
            };

            bounding_boxes.push(Some(child_bounds));
            child.layout_as_root(child_size.into(), window, cx);
            child.prepaint_at(origin, window, cx);

            origin = origin.apply_along(self.axis, |val| val + child_size.along(self.axis));

            let is_leaf_pane = self.is_leaf_pane_mask.get(ix).copied().unwrap_or(true);

            layout.children.push(PaneAxisChildLayout {
                bounds: child_bounds,
                element: child,
                handle: None,
                is_leaf_pane,
            })
        }

        for (ix, child_layout) in layout.children.iter_mut().enumerate() {
            if ix < len - 1 {
                child_layout.handle = Some(Self::layout_handle(
                    self.axis,
                    child_layout.bounds,
                    window,
                    cx,
                ));
            }
        }

        layout
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<ui::prelude::Pixels>,
        _: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for child in &mut layout.children {
            child.element.paint(window, cx);
        }

        let overlay_opacity = WorkspaceSettings::get(None, cx)
            .active_pane_modifiers
            .inactive_opacity
            .map(|val| val.0.clamp(0.0, 1.0))
            .and_then(|val| (val <= 1.).then_some(val));

        let mut overlay_background = cx.theme().colors().editor_background;
        if let Some(opacity) = overlay_opacity {
            overlay_background.fade_out(opacity);
        }

        let overlay_border = WorkspaceSettings::get(None, cx)
            .active_pane_modifiers
            .border_size
            .and_then(|val| (val >= 0.).then_some(val));

        for (ix, child) in &mut layout.children.iter_mut().enumerate() {
            if overlay_opacity.is_some() || overlay_border.is_some() {
                // the overlay has to be painted in origin+1px with size width-1px
                // in order to accommodate the divider between panels
                let overlay_bounds = Bounds {
                    origin: child
                        .bounds
                        .origin
                        .apply_along(Axis::Horizontal, |val| val + px(1.)),
                    size: child
                        .bounds
                        .size
                        .apply_along(Axis::Horizontal, |val| val - px(1.)),
                };

                if overlay_opacity.is_some()
                    && child.is_leaf_pane
                    && self.active_pane_ix != Some(ix)
                {
                    window.paint_quad(gpui::fill(overlay_bounds, overlay_background));
                }

                if let Some(border) = overlay_border
                    && self.active_pane_ix == Some(ix)
                    && child.is_leaf_pane
                {
                    window.paint_quad(gpui::quad(
                        overlay_bounds,
                        0.,
                        gpui::transparent_black(),
                        border,
                        cx.theme().colors().border_selected,
                        BorderStyle::Solid,
                    ));
                }
            }

            if let Some(handle) = child.handle.as_mut() {
                let cursor_style = match self.axis {
                    Axis::Vertical => CursorStyle::ResizeRow,
                    Axis::Horizontal => CursorStyle::ResizeColumn,
                };

                if layout
                    .dragged_handle
                    .borrow()
                    .is_some_and(|dragged_ix| dragged_ix == ix)
                {
                    window.set_window_cursor_style(cursor_style);
                } else {
                    window.set_cursor_style(cursor_style, &handle.hitbox);
                }

                window.paint_quad(gpui::fill(
                    handle.divider_bounds,
                    cx.theme().colors().pane_group_border,
                ));

                window.on_mouse_event({
                    let dragged_handle = layout.dragged_handle.clone();
                    let flexes = self.flexes.clone();
                    let workspace = self.workspace.clone();
                    let handle_hitbox = handle.hitbox.clone();
                    move |e: &MouseDownEvent, phase, window, cx| {
                        if phase.bubble() && handle_hitbox.is_hovered(window) {
                            dragged_handle.replace(Some(ix));
                            if e.click_count >= 2 {
                                let mut borrow = flexes.lock();
                                *borrow = vec![1.; borrow.len()];
                                workspace
                                    .update(cx, |this, cx| this.serialize_workspace(window, cx))
                                    .log_err();

                                window.refresh();
                            }
                            cx.stop_propagation();
                        }
                    }
                });
                window.on_mouse_event({
                    let workspace = self.workspace.clone();
                    let dragged_handle = layout.dragged_handle.clone();
                    let flexes = self.flexes.clone();
                    let child_bounds = child.bounds;
                    let axis = self.axis;
                    move |e: &MouseMoveEvent, phase, window, cx| {
                        let dragged_handle = dragged_handle.borrow();
                        if phase.bubble() && *dragged_handle == Some(ix) {
                            Self::compute_resize(
                                &flexes,
                                e,
                                ix,
                                axis,
                                child_bounds.origin,
                                bounds.size,
                                workspace.clone(),
                                window,
                                cx,
                            )
                        }
                    }
                });
            }
        }

        window.on_mouse_event({
            let dragged_handle = layout.dragged_handle.clone();
            move |_: &MouseUpEvent, phase, _window, _cx| {
                if phase.bubble() {
                    dragged_handle.replace(None);
                }
            }
        });
    }
}

impl ParentElement for PaneAxisElement {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

fn flex_values_in_bounds(flexes: &[f32]) -> bool {
    (flexes.iter().copied().sum::<f32>() - flexes.len() as f32).abs() < 0.001
}
