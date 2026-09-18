//! PanelButtons — 连接 Dock 和 StatusBar 的按钮视图。
//!
//! 对齐 zed `dock.rs:L390-L393` 的 `PanelButtons` struct。
//! Zed 的 PanelButtons 本身就是一个 StatusBar item（实现 `StatusItemView`），
//! 它关联一个 Dock entity，渲染成一排图标按钮。
//!
//! 每个按钮对应 Dock 里的一个 Panel（Arc<dyn PanelHandle>）：
//! - 点击已激活面板 → 关闭 dock
//! - 点击其他面板 → 打开 dock 并切 tab
//! - 按钮高亮 = dock 打开 + 该面板是 active_panel_index

use std::collections::HashMap;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use ui_gpui::component::context_menu::ContextMenuEntry;
use ui_gpui::component::divider::{Divider, DividerColor};
use ui_gpui::component::tooltip::Tooltip;
use ui_gpui::{IconButton, right_click_menu};

use crate::dock::Dock;
use crate::status_bar::StatusItemView;
use settings_content::DockPosition;

/// DockPosition 显示名称（对齐 zed `DockPosition::label()`，在 workspace 层）。
fn dock_label(p: DockPosition) -> &'static str {
    match p {
        DockPosition::Left => "Left",
        DockPosition::Bottom => "Bottom",
        DockPosition::Right => "Right",
    }
}

/// 状态栏上的一排面板按钮。关联一个 Dock entity，读它的 panels() 渲染。
pub struct PanelButtons {
    /// 自己的 dock
    dock: Entity<Dock>,
    /// 所有 dock，按 position 索引
    all_docks: HashMap<DockPosition, Entity<Dock>>,
}

impl PanelButtons {
    pub fn new(
        dock: Entity<Dock>,
        all_docks: HashMap<DockPosition, Entity<Dock>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&dock, |_, _, cx| cx.notify()).detach();
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

        // 每个面板按钮：click → 开关
        // 右键菜单的跨 Dock 搬面板暂时简化（PanelHandle::position_is_valid 需要 App 上下文）
        let mut buttons: Vec<gpui::AnyElement> = dock
            .panels()
            .iter()
            .enumerate()
            .map(|(i, panel)| {
                let is_active = dock_open && active_index == Some(i);
                let icon = panel.icon(cx);
                let tooltip = panel.icon_tooltip(cx);
                let dock_for_click = dock_entity.clone();
                let persistent_name = panel.persistent_name();
                let button_id: gpui::SharedString =
                    format!("panel-btn-{dock_position:?}-{i}").into();

                IconButton::new(button_id.clone(), icon)
                    .size(px(22.0))
                    .icon_size(px(14.0))
                    .radius(ui_gpui::ButtonRadius::Medium)
                    .aria_label(tooltip)
                    .selected(is_active)
                    .tooltip(Tooltip::text(tooltip.to_string()))
                    .tooltip_anchor(gpui::Anchor::BottomLeft)
                    .tooltip_attach(gpui::Anchor::TopLeft)
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
            .collect();

        // Zed 右侧 dock 的按钮反序（先加的靠右）
        if dock_position == DockPosition::Right {
            buttons.reverse();
        }

        let has_buttons = !buttons.is_empty();

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .when(
                has_buttons && matches!(dock_position, DockPosition::Right | DockPosition::Bottom),
                |this| this.child(Divider::vertical().color(DividerColor::Border)),
            )
            .children(buttons)
            .when(has_buttons && dock_position == DockPosition::Left, |this| {
                this.child(Divider::vertical().color(DividerColor::Border))
            })
            .into_any_element()
    }
}
