//! PaneGroup — 递归 split 树，对齐 zed `crates/workspace/src/pane_group.rs`。
//!
//! Zed PaneGroup 是普通 struct（非 GPUI entity），AAgent 保持一致。
//! - `Member::Pane(Entity<Pane>)` — 叶子
//! - `Member::PaneGroup(PaneGroup)` — 内部节点（可以继续 split）
//! - `axis: Axis` — 叶子们怎么排（Horizontal=上下, Vertical=左右）

use gpui::{AnyElement, App, Axis, Entity, IntoElement, div, prelude::*};

use crate::pane::Pane;

/// PaneGroup 的一个 child。
#[derive(Clone)]
pub enum Member {
    /// 叶子 Pane。
    Pane(Entity<Pane>),
    /// 嵌套 PaneGroup（继续 split）。
    PaneGroup(Box<PaneGroup>),
}

/// One or many panes arranged in horizontal or vertical axis.
/// 单 PaneGroup 就是根只有一个 Member::Pane 的 PaneGroup。
#[derive(Clone)]
pub struct PaneGroup {
    pub axis: Axis,
    pub members: Vec<Member>,
    /// 每个 member 的 flex 比例（和为 1.0）。
    pub flex_ratios: Vec<f32>,
}

impl PaneGroup {
    /// 单 pane 起步。
    pub fn new(pane: Entity<Pane>) -> Self {
        Self {
            axis: Axis::Vertical,
            members: vec![Member::Pane(pane)],
            flex_ratios: vec![1.0],
        }
    }

    /// 在 member index 位置 split 一个新 pane。
    pub fn split(&mut self, index: usize, new_pane: Entity<Pane>, direction: Axis) {
        if self.members.len() <= 1 {
            // 单成员 — 直接在当前层 split
            self.axis = direction;
            self.members.insert(index, Member::Pane(new_pane));
            self.flex_ratios.insert(index, 0.5);
            // 重新均分
            let n = self.members.len();
            self.flex_ratios = vec![1.0 / n as f32; n];
        } else if self.axis == direction {
            // 同方向 — 直接插
            self.members.insert(index, Member::Pane(new_pane));
            let n = self.members.len();
            self.flex_ratios = vec![1.0 / n as f32; n];
        } else {
            // 异方向 — 把当前 member 包成子 PaneGroup
            let old = self.members.remove(index);
            let old_ratio = self.flex_ratios.remove(index);
            let child = PaneGroup {
                axis: direction,
                members: vec![old, Member::Pane(new_pane)],
                flex_ratios: vec![0.5, 0.5],
            };
            self.members
                .insert(index, Member::PaneGroup(Box::new(child)));
            self.flex_ratios.insert(index, old_ratio);
        }
    }

    /// 递归移除空 Pane。
    pub fn prune(&mut self, cx: &App) {
        self.members.retain(|m| match m {
            Member::Pane(e) => !e.read(cx).is_empty(),
            Member::PaneGroup(g) => {
                let mut g = Box::new(PaneGroup {
                    axis: g.axis,
                    members: g.members.clone(),
                    flex_ratios: g.flex_ratios.clone(),
                });
                g.prune(cx);
                !g.members.is_empty()
            }
        });
        if self.members.len() == 1 {
            if let Member::PaneGroup(g) = &self.members[0] {
                let inner = *g.clone();
                self.axis = inner.axis;
                self.members = inner.members;
                self.flex_ratios = inner.flex_ratios;
            }
        }
        let n = self.members.len().max(1);
        self.flex_ratios = vec![1.0 / n as f32; n];
    }

    /// 渲染树 — 每个 Entity<Pane> 自己实现 Render，GPUI 框架递归调。
    pub fn render(&self) -> AnyElement {
        if self.members.is_empty() {
            return div().into_any_element();
        }
        if self.members.len() == 1 {
            return match &self.members[0] {
                Member::Pane(e) => e.clone().into_any_element(),
                Member::PaneGroup(g) => g.render(),
            };
        }

        let mut container = div().size_full();
        container = match self.axis {
            Axis::Vertical => container.flex_col(),
            Axis::Horizontal => container.flex_row(),
        };

        for (member, ratio) in self.members.iter().zip(self.flex_ratios.iter()) {
            let element = match member {
                Member::Pane(e) => e.clone().into_any_element(),
                Member::PaneGroup(g) => g.render(),
            };
            container = container.child(
                div()
                    .flex_grow(*ratio)
                    .flex_shrink(1.0)
                    .flex_basis(gpui::relative(0.))
                    .child(element),
            );
        }
        container.into_any_element()
    }

    /// 找到最后一个 pane（用于 NewCenterTerminal）。
    pub fn last_pane(&self) -> Option<Entity<Pane>> {
        self.members.last().and_then(|m| match m {
            Member::Pane(e) => Some(e.clone()),
            Member::PaneGroup(g) => g.last_pane(),
        })
    }
}
