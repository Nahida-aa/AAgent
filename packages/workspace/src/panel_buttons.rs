//! PanelButtons — 连接 Dock 和 StatusBar 的按钮视图。
//!
//! 对齐 zed `dock.rs:L390-L393` 的 `PanelButtons` struct。
//! Zed 的 PanelButtons 本身就是一个 StatusBar item（实现 `StatusItemView`），
//! 它关联一个 Dock entity，渲染成一排图标按钮。
//!
//! 每个按钮对应 Dock 里的一个面板：
//! - 点击已激活面板 → 关闭 dock
//! - 点击其他面板 → 打开 dock 并切 tab
//! - 按钮高亮 = dock 打开 + 该面板是 active_panel_index（用 IconButton::selected）
//! - 右键 → Dock Left / Right / Bottom 切换（把面板搬到目标 Dock）

use std::collections::HashMap;

use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use ui_gpui::IconButton;
use ui_gpui::component::context_menu::ContextMenuEntry;
use ui_gpui::component::tooltip::Tooltip;
use ui_gpui::right_click_menu;

use crate::dock::Dock;
use crate::dock_position::DockPosition;
use crate::status_bar::StatusItemView;

/// 状态栏上的一排面板按钮。关联一个 Dock entity，读它的 panel_entries 渲染。
/// 同时持有所有 3 个 Dock 的引用（用于右键菜单搬面板）。
pub struct PanelButtons {
    /// 自己的 dock（读 panel_entries 渲染按钮）
    dock: Entity<Dock>,
    /// 所有 dock，按 position 索引（用于跨 Dock 搬面板）
    all_docks: HashMap<DockPosition, Entity<Dock>>,
}

impl PanelButtons {
    pub fn new(
        dock: Entity<Dock>,
        all_docks: HashMap<DockPosition, Entity<Dock>>,
        cx: &mut Context<Self>,
    ) -> Self {
        // 订阅 dock 变化（面板增删、active 切换、开关）→ 自动重新渲染
        cx.observe(&dock, |_, _, cx| cx.notify()).detach();
        // 也要订阅其他 dock（面板搬进来时需要刷新）
        for d in all_docks.values() {
            if d != &dock {
                let d = d.clone();
                cx.observe(&d, |_, _, cx| cx.notify()).detach();
            }
        }
        Self { dock, all_docks }
    }

    pub fn dock(&self) -> &Entity<Dock> {
        &self.dock
    }
}

impl StatusItemView for PanelButtons {}

impl Render for PanelButtons {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let dock = self.dock.read(cx);
        let dock_open = dock.is_open();
        let active_index = dock.active_panel_index();
        let dock_position = dock.position();
        let dock_entity = self.dock.clone();
        let all_docks = self.all_docks.clone();

        // 每个面板按钮：click → 开关；right-click → 搬 Dock
        let mut buttons: Vec<gpui::AnyElement> = dock
            .panel_entries()
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_active = dock_open && active_index == Some(i);
                let icon = entry.icon();
                let label = entry.icon_tooltip();
                let entry_label = label.clone();

                let dock_for_click = dock_entity.clone();

                // 右键菜单：Dock Left / Right / Bottom 切换
                let entry_for_menu = entry.clone();
                let current_pos = dock_position;
                let docks_for_menu = all_docks.clone();

                let menu_id: gpui::SharedString =
                    format!("panel-btn-menu-{dock_position:?}-{i}").into();

                right_click_menu::<ui_gpui::ContextMenu>(menu_id)
                    .trigger(move |_is_active, _window, _cx| {
                        IconButton::new(format!("panel-btn-{dock_position:?}-{i}"), icon)
                            .size(px(22.0))
                            .icon_size(px(14.0))
                            .radius(ui_gpui::ButtonRadius::Medium)
                            .aria_label(label)
                            .selected(is_active)
                            .tooltip(Tooltip::text(entry_label.to_string()))
                            .on_click(move |_ev, _window, cx| {
                                dock_for_click.update(cx, |dock, cx| {
                                    if dock.is_open() && dock.active_panel_index() == Some(i) {
                                        dock.set_open(false);
                                    } else {
                                        dock.set_open(true);
                                        dock.activate_panel(i);
                                    }
                                    cx.notify();
                                });
                            })
                            .into_any_element()
                    })
                    .menu(move |_window, cx| {
                        let entry = entry_for_menu.clone();
                        let pos = current_pos;
                        let docks = docks_for_menu.clone();

                        let positions: [DockPosition; 3] = [
                            DockPosition::Left,
                            DockPosition::Right,
                            DockPosition::Bottom,
                        ];

                        ui_gpui::ContextMenu::build(cx, move |menu, _| {
                            let mut menu = menu;
                            for target in positions {
                                let is_current = target == pos;
                                let entry = entry.clone();
                                let docks = docks.clone();
                                menu = menu.item(
                                    ContextMenuEntry::new(format!("Dock {}", target.label()))
                                        .checked(is_current)
                                        .on_click(move |_window, cx| {
                                            if is_current {
                                                return;
                                            }
                                            // 从当前 dock 移除，加到目标 dock
                                            let entry = entry.clone();
                                            let from_pos = pos;
                                            let docks = docks.clone();
                                            if let (Some(target), Some(current)) =
                                                (docks.get(&target), docks.get(&from_pos))
                                            {
                                                current.update(cx, |dock, cx| {
                                                    let idx = dock
                                                        .panel_entries()
                                                        .iter()
                                                        .position(|e| e.kind == entry.kind);
                                                    if let Some(idx) = idx {
                                                        let removed = dock.remove_panel(idx);
                                                        if let Some(e) = removed {
                                                            target.clone().update(cx, |td, cx| {
                                                                td.add_panel(e);
                                                                td.set_open(true);
                                                                cx.notify();
                                                            });
                                                        }
                                                    }
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                );
                            }
                            menu
                        })
                    })
                    .into_any_element()
            })
            .collect();

        // Zed 右侧 dock 的按钮反序（先加的靠右）
        if dock_position == DockPosition::Right {
            buttons.reverse();
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .children(buttons)
            .into_any_element()
    }
}
