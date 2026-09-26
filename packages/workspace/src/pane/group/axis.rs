use std::sync::Arc;

use anyhow::Result;
// `IntoElement` / `ParentElement` 必须在作用域里：`result.element.into_any_element()`
// 与 `pane_axis(..).children(..)` 都是这两个 trait 的方法，不由 `ui::prelude::*`
// 带进来。缺 `ParentElement` 时 `.children(..)` 会被解析成 `PaneAxisElement` 的
// 私有同名字段，报 "private field, not a method"（很误导）。
use gpui::{
    Along, AnyWeakView, App, Axis, Bounds, Entity, IntoElement, ParentElement, Pixels, Point,
    WeakEntity, Window, px,
};
use parking_lot::Mutex;

use crate::Pane;

use super::member::Member;
use super::render::{PaneLeaderDecorator, PaneRenderResult};
use super::split_direction::SplitDirection;
use super::{pane_axis, HORIZONTAL_MIN_SIZE, VERTICAL_MIN_SIZE};

#[derive(Debug, Clone)]
pub struct PaneAxis {
    pub axis: Axis,
    pub members: Vec<Member>,
    pub flexes: Arc<Mutex<Vec<f32>>>,
    pub bounding_boxes: Arc<Mutex<Vec<Option<Bounds<Pixels>>>>>,
}

impl PaneAxis {
    pub fn new(axis: Axis, members: Vec<Member>) -> Self {
        let flexes = Arc::new(Mutex::new(vec![1.; members.len()]));
        let bounding_boxes = Arc::new(Mutex::new(vec![None; members.len()]));
        Self {
            axis,
            members,
            flexes,
            bounding_boxes,
        }
    }

    pub fn load(axis: Axis, members: Vec<Member>, flexes: Option<Vec<f32>>) -> Self {
        let mut flexes = flexes.unwrap_or_else(|| vec![1.; members.len()]);
        if flexes.len() != members.len()
            || (flexes.iter().copied().sum::<f32>() - flexes.len() as f32).abs() >= 0.001
        {
            flexes = vec![1.; members.len()];
        }

        let flexes = Arc::new(Mutex::new(flexes));
        let bounding_boxes = Arc::new(Mutex::new(vec![None; members.len()]));
        Self {
            axis,
            members,
            flexes,
            bounding_boxes,
        }
    }

    pub(super) fn split(
        &mut self,
        old_pane: &Entity<Pane>,
        new_pane: &Entity<Pane>,
        direction: SplitDirection,
    ) -> bool {
        for (mut idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if axis.split(old_pane, new_pane, direction) {
                        return true;
                    }
                }
                Member::Pane(pane) => {
                    if pane == old_pane {
                        if direction.axis() == self.axis {
                            if direction.increasing() {
                                idx += 1;
                            }
                            self.insert_pane(idx, new_pane);
                        } else {
                            *member =
                                Member::new_axis(old_pane.clone(), new_pane.clone(), direction);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn insert_pane(&mut self, idx: usize, new_pane: &Entity<Pane>) {
        self.members.insert(idx, Member::Pane(new_pane.clone()));
        *self.flexes.lock() = vec![1.; self.members.len()];
    }

    pub(super) fn find_pane_at_border(&self, direction: SplitDirection) -> Option<&Entity<Pane>> {
        if self.axis != direction.axis() {
            return None;
        }
        let member = if direction.increasing() {
            self.members.last()
        } else {
            self.members.first()
        };
        member.and_then(|e| match e {
            Member::Pane(pane) => Some(pane),
            Member::Axis(_) => None,
        })
    }

    pub(super) fn remove(&mut self, pane_to_remove: &Entity<Pane>) -> Result<Option<Member>> {
        let mut found_pane = false;
        let mut remove_member = None;
        for (idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if let Ok(last_pane) = axis.remove(pane_to_remove) {
                        if let Some(last_pane) = last_pane {
                            *member = last_pane;
                        }
                        found_pane = true;
                        break;
                    }
                }
                Member::Pane(pane) => {
                    if pane == pane_to_remove {
                        found_pane = true;
                        remove_member = Some(idx);
                        break;
                    }
                }
            }
        }

        if found_pane {
            if let Some(idx) = remove_member {
                self.members.remove(idx);
                *self.flexes.lock() = vec![1.; self.members.len()];
            }

            if self.members.len() == 1 {
                let result = self.members.pop();
                *self.flexes.lock() = vec![1.; self.members.len()];
                Ok(result)
            } else {
                Ok(None)
            }
        } else {
            anyhow::bail!("Pane not found");
        }
    }

    pub(super) fn reset_pane_sizes(&self) {
        *self.flexes.lock() = vec![1.; self.members.len()];
        for member in self.members.iter() {
            if let Member::Axis(axis) = member {
                axis.reset_pane_sizes();
            }
        }
    }

    pub(super) fn resize(
        &mut self,
        pane: &Entity<Pane>,
        axis: Axis,
        amount: Pixels,
        bounds: &Bounds<Pixels>,
    ) -> Option<bool> {
        let container_size = self
            .bounding_boxes
            .lock()
            .iter()
            .filter_map(|e| *e)
            .reduce(|acc, e| acc.union(&e))
            .unwrap_or(*bounds)
            .size;

        let found_pane = self
            .members
            .iter()
            .any(|member| matches!(member, Member::Pane(p) if p == pane));

        if found_pane && self.axis != axis {
            return Some(false); // pane found but this is not the correct axis direction
        }
        let mut found_axis_index: Option<usize> = None;
        if !found_pane {
            for (i, pa) in self.members.iter_mut().enumerate() {
                if let Member::Axis(pa) = pa
                    && let Some(done) = pa.resize(pane, axis, amount, bounds)
                {
                    if done {
                        return Some(true); // pane found and operations already done
                    } else if self.axis != axis {
                        return Some(false); // pane found but this is not the correct axis direction
                    } else {
                        found_axis_index = Some(i); // pane found and this is correct direction
                    }
                }
            }
            found_axis_index?; // no pane found
        }

        let min_size = match axis {
            Axis::Horizontal => px(HORIZONTAL_MIN_SIZE),
            Axis::Vertical => px(VERTICAL_MIN_SIZE),
        };
        let mut flexes = self.flexes.lock();

        let ix = if found_pane {
            self.members.iter().position(|m| {
                if let Member::Pane(p) = m {
                    p == pane
                } else {
                    false
                }
            })
        } else {
            found_axis_index
        };

        if ix.is_none() {
            return Some(true);
        }

        let ix = ix.unwrap_or(0);

        let size = move |ix, flexes: &[f32]| {
            container_size.along(axis) * (flexes[ix] / flexes.len() as f32)
        };

        // Don't allow resizing to less than the minimum size, if elements are already too small
        if min_size - px(1.) > size(ix, flexes.as_slice()) {
            return Some(true);
        }

        let flex_changes = |pixel_dx, target_ix, next: isize, flexes: &[f32]| {
            let flex_change = flexes.len() as f32 * pixel_dx / container_size.along(axis);
            let current_target_flex = flexes[target_ix] + flex_change;
            let next_target_flex = flexes[(target_ix as isize + next) as usize] - flex_change;
            (current_target_flex, next_target_flex)
        };

        let apply_changes =
            |current_ix: usize, proposed_current_pixel_change: Pixels, flexes: &mut [f32]| {
                let next_target_size = Pixels::max(
                    size(current_ix + 1, flexes) - proposed_current_pixel_change,
                    min_size,
                );
                let current_target_size = Pixels::max(
                    size(current_ix, flexes) + size(current_ix + 1, flexes) - next_target_size,
                    min_size,
                );

                let current_pixel_change = current_target_size - size(current_ix, flexes);

                let (current_target_flex, next_target_flex) =
                    flex_changes(current_pixel_change, current_ix, 1, flexes);

                flexes[current_ix] = current_target_flex;
                flexes[current_ix + 1] = next_target_flex;
            };

        if ix + 1 == flexes.len() {
            apply_changes(ix - 1, -1.0 * amount, flexes.as_mut_slice());
        } else {
            apply_changes(ix, amount, flexes.as_mut_slice());
        }
        Some(true)
    }

    pub(super) fn swap(&mut self, from: &Entity<Pane>, to: &Entity<Pane>) {
        for member in self.members.iter_mut() {
            match member {
                Member::Axis(axis) => axis.swap(from, to),
                Member::Pane(pane) => {
                    if pane == from {
                        *member = Member::Pane(to.clone());
                    } else if pane == to {
                        *member = Member::Pane(from.clone())
                    }
                }
            }
        }
    }

    pub(super) fn bounding_box_for_pane(&self, pane: &Entity<Pane>) -> Option<Bounds<Pixels>> {
        debug_assert!(self.members.len() == self.bounding_boxes.lock().len());

        for (idx, member) in self.members.iter().enumerate() {
            match member {
                Member::Pane(found) => {
                    if pane == found {
                        return self.bounding_boxes.lock()[idx];
                    }
                }
                Member::Axis(axis) => {
                    if let Some(rect) = axis.bounding_box_for_pane(pane) {
                        return Some(rect);
                    }
                }
            }
        }
        None
    }

    pub(super) fn pane_at_pixel_position(
        &self,
        coordinate: Point<Pixels>,
    ) -> Option<&Entity<Pane>> {
        debug_assert!(self.members.len() == self.bounding_boxes.lock().len());

        let bounding_boxes = self.bounding_boxes.lock();

        for (idx, member) in self.members.iter().enumerate() {
            if let Some(coordinates) = bounding_boxes[idx]
                && coordinates.contains(&coordinate)
            {
                return match member {
                    Member::Pane(found) => Some(found),
                    Member::Axis(axis) => axis.pane_at_pixel_position(coordinate),
                };
            }
        }
        None
    }

    pub(super) fn full_height_column_count(&self) -> usize {
        match self.axis {
            Axis::Horizontal => self
                .members
                .iter()
                .map(Member::full_height_column_count)
                .sum::<usize>()
                .max(1),
            Axis::Vertical => self
                .members
                .iter()
                .map(Member::full_height_column_count)
                .max()
                .unwrap_or(1),
        }
    }

    pub(super) fn render(
        &self,
        basis: usize,
        zoomed: Option<&AnyWeakView>,
        maximized: Option<&WeakEntity<Pane>>,
        render_cx: &dyn PaneLeaderDecorator,
        window: &mut Window,
        cx: &mut App,
    ) -> PaneRenderResult {
        if let Some(maximized) = maximized {
            if let Some(maximized_pane) = maximized.upgrade() {
                for member in &self.members {
                    if member.contains_pane(&maximized_pane) {
                        return member.render(
                            basis,
                            zoomed,
                            Some(maximized),
                            render_cx,
                            window,
                            cx,
                        );
                    }
                }
            }
        }

        debug_assert!(self.members.len() == self.flexes.lock().len());
        let mut active_pane_ix = None;
        let mut contains_active_pane = false;
        let mut is_leaf_pane = vec![false; self.members.len()];

        let rendered_children = self
            .members
            .iter()
            .enumerate()
            .map(|(ix, member)| {
                match member {
                    Member::Pane(pane) => {
                        is_leaf_pane[ix] = true;
                        if pane == render_cx.active_pane() {
                            active_pane_ix = pane.read(cx).has_focus(window, cx).then_some(ix);
                            contains_active_pane = true;
                        }
                    }
                    Member::Axis(_) => {
                        is_leaf_pane[ix] = false;
                    }
                }

                let result = member.render((basis + ix) * 10, zoomed, None, render_cx, window, cx);
                if result.contains_active_pane {
                    contains_active_pane = true;
                }
                result.element.into_any_element()
            })
            .collect::<Vec<_>>();

        let element = pane_axis(
            self.axis,
            basis,
            self.flexes.clone(),
            self.bounding_boxes.clone(),
            render_cx.workspace().clone(),
        )
        .with_is_leaf_pane_mask(is_leaf_pane)
        .children(rendered_children)
        .with_active_pane(active_pane_ix)
        .into_any_element();

        PaneRenderResult {
            element,
            contains_active_pane,
            #[cfg(any(test, feature = "test-support"))]
            decorated_pane_ix: active_pane_ix,
        }
    }
}
