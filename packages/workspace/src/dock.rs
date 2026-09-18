//! Dock 面板容器，对齐 zed `dock.rs`。

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Window, div, hsla, prelude::*, px,
};
use ui_gpui::IconButton;
use ui_gpui::theme::ActiveTheme;

use crate::dock_position::DockPosition;
use crate::panel::{PanelEntry, PanelKind};

/// 面板容器（对齐 zed `pub struct Dock` dock.rs:L283-L295）。
pub struct Dock {
    position: DockPosition,
    panel_entries: Vec<PanelEntry>,
    is_open: bool,
    active_panel_index: Option<usize>,
}

impl Dock {
    pub fn new(position: DockPosition) -> Self {
        Self {
            position,
            panel_entries: Vec::new(),
            is_open: false,
            active_panel_index: None,
        }
    }

    pub fn position(&self) -> DockPosition {
        self.position
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn active_panel_index(&self) -> Option<usize> {
        self.active_panel_index
    }

    pub fn panel_entries(&self) -> &[PanelEntry] {
        &self.panel_entries
    }

    pub fn add_panel(&mut self, entry: PanelEntry) {
        self.panel_entries.push(entry);
        if self.active_panel_index.is_none() {
            self.active_panel_index = Some(0);
        }
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.is_open = false;
        } else {
            self.is_open = true;
            if self.active_panel_index.is_none() && !self.panel_entries.is_empty() {
                self.active_panel_index = Some(0);
            }
        }
    }

    pub fn activate_panel(&mut self, index: usize) {
        if index < self.panel_entries.len() {
            self.active_panel_index = Some(index);
        }
    }

    /// 移除并返回 index 的面板（用于跨 Dock 移动）。
    pub fn remove_panel(&mut self, index: usize) -> Option<PanelEntry> {
        if index < self.panel_entries.len() {
            let removed = self.panel_entries.remove(index);
            // 修正 active_panel_index
            match self.active_panel_index {
                Some(ai) if ai == index => {
                    self.active_panel_index = self
                        .panel_entries
                        .get(ai)
                        .map(|_| ai)
                        .or(self.panel_entries.first().map(|_| 0));
                }
                Some(ai) if ai > index => {
                    self.active_panel_index = Some(ai - 1);
                }
                _ => {}
            }
            if self.panel_entries.is_empty() {
                self.active_panel_index = None;
            }
            Some(removed)
        } else {
            None
        }
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.is_open || self.panel_entries.is_empty() {
            return div().into_any_element();
        }

        let colors = cx.theme().colors();
        let position = self.position;
        let dock = cx.entity();

        let axis = position.axis();

        let mut root = div()
            .id("dock-panel")
            .w_full()
            .h_full()
            .bg(colors.panel_background)
            .border_color(colors.border)
            .overflow_hidden()
            .map(|el| match axis {
                gpui::Axis::Horizontal => el.flex_col(),
                gpui::Axis::Vertical => el.flex_row(),
            })
            .map(|el| match position {
                DockPosition::Left => el.border_r_1(),
                DockPosition::Right => el.border_l_1(),
                DockPosition::Bottom => el.border_t_1(),
                _ => el,
            });

        // Tab bar
        let mut tab_bar = div().flex_row().items_center().gap_1().px_1().py_0p5();
        for (i, entry) in self.panel_entries.iter().enumerate() {
            let is_active = Some(i) == self.active_panel_index;
            let icon = entry.icon();
            let label = entry.icon_tooltip();
            let dock = dock.clone();
            tab_bar = tab_bar.child(
                IconButton::new(format!("dock-tab-{}-{}", self.position.label(), i), icon)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ui_gpui::ButtonRadius::Medium)
                    .aria_label(label)
                    .selected(is_active)
                    .on_click(move |_ev, _window, cx| {
                        dock.update(cx, |dock, cx| {
                            dock.activate_panel(i);
                            cx.notify();
                        });
                    }),
            );
        }

        // Active panel content — 占位
        let active_kind = self
            .active_panel_index
            .and_then(|i| self.panel_entries.get(i))
            .map(|e| e.kind);

        let content = match active_kind {
            Some(kind) => div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_size(px(16.0))
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child(format!("{} (placeholder)", kind.aria_label()))
                .into_any_element(),
            None => gpui::Empty.into_any_element(),
        };

        root.child(tab_bar).child(content).into_any_element()
    }
}
