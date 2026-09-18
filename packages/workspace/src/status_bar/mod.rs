//! 应用底部状态栏（status bar），结构对齐 zed `workspace::status_bar`。
//!
//! Zed 的状态栏是一排可增删的「项」（`StatusItemView`），左右两组
//! （`add_left_item` / `add_right_item`），右侧渲染时反序（先加的靠右、
//! 后加的靠左）。
//!
//! Sidebar 状态由 Sidebar entity 独立管理，但 StatusBar 需要**缓存**它来
//! 渲染 sidebar toggle 按钮（sidebar 关闭时才显示）。MultiWorkspace observe
//! sidebar 后同步给 Workspace → StatusBar。
//!
//! - [`StatusItemView`]：item trait（对齐 zed）。
//! - [`StatusBar`]：容器，左侧/右侧各渲染一条 `h_flex`。

pub mod items;

use std::any::TypeId;
use std::collections::HashSet;

use gpui::{
    AnyView, Context, Entity, IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use ui_gpui::theme::ActiveTheme;
use ui_gpui::{
    ButtonRadius, ContextMenu, ContextMenuEntry, Divider, DividerColor, IconButton, IconName,
    Tooltip, right_click_menu,
};

use crate::dock::panel_buttons::PanelButtons;
use crate::sidebar::SidebarStatus;
use settings_content::DockPosition;

/// 状态栏项（对齐 zed `StatusItemView`）。
pub trait StatusItemView: Render {
    fn has_custom_menu(&self) -> bool {
        false
    }
}

/// trait 对象化的 item handle。
pub trait StatusItemViewHandle: Send + Sync {
    fn to_any(&self) -> AnyView;
    fn item_type(&self) -> TypeId;
    fn has_custom_menu(&self) -> bool;
}

impl<T: StatusItemView> StatusItemViewHandle for Entity<T> {
    fn to_any(&self) -> AnyView {
        self.clone().into()
    }
    fn item_type(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn has_custom_menu(&self) -> bool {
        false
    }
}

/// 底部状态栏。
///
/// 渲染: [sidebar-toggle?] [left-items...] | [right-items...] [sidebar-toggle?]
///
/// 左侧 sidebar 关闭且在左侧时，左侧显示 toggle。
/// 右侧 sidebar 关闭且在右侧时，右侧显示 toggle（反序渲染所以在最右）。
pub struct StatusBar {
    left_items: Vec<Box<dyn StatusItemViewHandle>>,
    right_items: Vec<Box<dyn StatusItemViewHandle>>,
    hidden_items: HashSet<TypeId>,
    /// 缓存的 sidebar 状态 — 也直接持有 Sidebar entity 的弱引用，
    /// toggle 时直接同步（避免 StatusBar 缓存和 Sidebar entity 状态不一致）。
    sidebar: SidebarStatus,
    sidebar_entity: Option<WeakEntity<crate::sidebar::Sidebar>>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            left_items: Vec::new(),
            right_items: Vec::new(),
            hidden_items: HashSet::new(),
            sidebar: SidebarStatus::default(),
            sidebar_entity: None,
        }
    }

    /// 设置 Sidebar entity 弱引用 — MultiWorkspace 创建后通过 Workspace 传入。
    pub fn set_sidebar_entity(&mut self, sidebar: Entity<crate::sidebar::Sidebar>) {
        self.sidebar_entity = Some(sidebar.downgrade());
    }

    pub fn add_left_item<T: StatusItemView>(&mut self, item: Entity<T>) {
        self.left_items.push(Box::new(item));
    }

    pub fn add_right_item<T: StatusItemView>(&mut self, item: Entity<T>) {
        self.right_items.push(Box::new(item));
    }

    pub fn remove_item_of_type<T: StatusItemView>(&mut self) {
        self.left_items
            .retain(|item| item.item_type() != TypeId::of::<T>());
        self.right_items
            .retain(|item| item.item_type() != TypeId::of::<T>());
    }

    pub fn hide_item(&mut self, item_type: TypeId) {
        self.hidden_items.insert(item_type);
    }

    pub fn sidebar(&self) -> SidebarStatus {
        self.sidebar
    }

    /// MultiWorkspace observe sidebar 后调用，保持 StatusBar 状态同步。
    pub fn set_sidebar(&mut self, status: SidebarStatus) {
        self.sidebar = status;
    }

    // ---- 渲染 ----

    fn visible_left_items(&self) -> impl Iterator<Item = &Box<dyn StatusItemViewHandle>> {
        self.left_items
            .iter()
            .filter(|item| !self.hidden_items.contains(&item.item_type()))
    }

    fn visible_right_items(&self) -> impl Iterator<Item = &Box<dyn StatusItemViewHandle>> {
        self.right_items
            .iter()
            .filter(|item| !self.hidden_items.contains(&item.item_type()))
    }

    /// sidebar toggle 按钮（sidebar 关闭时在 StatusBar 显示打开按钮）。
    /// 对齐 zed StatusBar::render_sidebar_toggle。
    fn render_sidebar_toggle(&self, on_right: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let current_side = self.sidebar.side;
        let bar = cx.entity();
        let divider = || Divider::vertical().color(DividerColor::Border);

        let (menu_anchor, menu_attach) = if on_right {
            (gpui::Anchor::BottomRight, gpui::Anchor::TopRight)
        } else {
            (gpui::Anchor::BottomLeft, gpui::Anchor::TopLeft)
        };

        let bar_for_click = bar.clone();
        let bar_for_menu = bar.clone();
        let toggle = right_click_menu::<ContextMenu>("sidebar-toggle-menu")
            .anchor(menu_anchor)
            .attach(menu_attach)
            .trigger(move |_is_active, _window, _cx| {
                let bar = bar_for_click.clone();
                IconButton::new(
                    if on_right {
                        "toggle-workspace-sidebar-right"
                    } else {
                        "toggle-workspace-sidebar-left"
                    },
                    if on_right {
                        IconName::ThreadsSidebarRightClosed
                    } else {
                        IconName::ThreadsSidebarLeftClosed
                    },
                )
                .size(px(22.0))
                .icon_size(px(14.0))
                .radius(ButtonRadius::Medium)
                .aria_label("Open Threads Sidebar")
                .tooltip(Tooltip::text("Open Threads Sidebar"))
                .tooltip_anchor(gpui::Anchor::BottomLeft)
                .tooltip_attach(gpui::Anchor::TopLeft)
                .on_click(move |_event, _window, cx| {
                    let side = if on_right {
                        DockPosition::Right
                    } else {
                        DockPosition::Left
                    };
                    bar.update(cx, |bar, cx| {
                        bar.toggle_sidebar(side, cx);
                    });
                })
                .into_any_element()
            })
            .menu(move |_window, cx| {
                let bar = bar_for_menu.clone();
                ContextMenu::build(cx, move |menu, _| {
                    let current = current_side;
                    let positions: [(DockPosition, &str); 2] =
                        [(DockPosition::Left, "Left"), (DockPosition::Right, "Right")];
                    let mut m = menu;
                    for (pos, label) in positions {
                        let bar = bar.clone();
                        let is_current = pos == current;
                        m = m.item(ContextMenuEntry::new(label).checked(is_current).on_click(
                            move |_window, cx| {
                                bar.update(cx, |bar, cx| bar.set_side(pos, cx));
                            },
                        ));
                    }
                    m
                })
            })
            .into_any_element();

        let mut children: Vec<gpui::AnyElement> = Vec::new();
        if on_right {
            children.push(divider().into_any_element());
        }
        children.push(toggle);
        if !on_right {
            children.push(divider().into_any_element());
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .children(children)
            .into_any_element()
    }

    /// 左组：[sidebar-toggle?] + Dock PanelButtons + 普通项。
    fn render_left_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        if !self.sidebar.open && self.sidebar.side == DockPosition::Left {
            children.push(self.render_sidebar_toggle(false, cx).into_any_element());
        }
        for (ix, item) in self.visible_left_items().enumerate() {
            if item.item_type() == TypeId::of::<PanelButtons>() {
                children.push(item.to_any().into_any_element());
            } else {
                let view = item.to_any();
                let item_type = item.item_type();
                let bar = cx.entity();
                let hideable = right_click_menu::<ContextMenu>(("hideable-item", ix))
                    .trigger(move |_is_active, _window, _cx| view.clone().into_any_element())
                    .menu(move |_window, cx| {
                        let bar = bar.clone();
                        ContextMenu::build(cx, |menu, _| {
                            menu.item(ContextMenuEntry::new("Hide").on_click(move |_window, cx| {
                                bar.update(cx, |bar, _| bar.hide_item(item_type));
                            }))
                        })
                    });
                children.push(hideable.into_any_element());
            }
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .min_w_0()
            .overflow_x_hidden()
            .children(children)
    }

    /// 右组：普通项 + Dock PanelButtons + [sidebar-toggle?]，反序渲染所以显示时 toggle 在最右。
    fn render_right_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        for (ix, item) in self.visible_right_items().enumerate() {
            if item.item_type() == TypeId::of::<PanelButtons>() {
                children.push(item.to_any().into_any_element());
            } else {
                let view = item.to_any();
                let item_type = item.item_type();
                let bar = cx.entity();
                let hideable = right_click_menu::<ContextMenu>(("hideable-item", ix))
                    .trigger(move |_is_active, _window, _cx| view.clone().into_any_element())
                    .menu(move |_window, cx| {
                        let bar = bar.clone();
                        ContextMenu::build(cx, |menu, _| {
                            menu.item(ContextMenuEntry::new("Hide").on_click(move |_window, cx| {
                                bar.update(cx, |bar, _| bar.hide_item(item_type));
                            }))
                        })
                    });
                children.push(hideable.into_any_element());
            }
        }
        if !self.sidebar.open && self.sidebar.side == DockPosition::Right {
            children.push(self.render_sidebar_toggle(true, cx).into_any_element());
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .children(children.into_iter().rev())
    }

    /// toggle sidebar — 直接调 Sidebar entity（通过弱引用升级）。
    /// Sidebar entity 改变后 MultiWorkspace 重渲染 sidebar。
    pub fn toggle_sidebar(&mut self, side: DockPosition, cx: &mut Context<Self>) {
        if side == DockPosition::Bottom {
            return;
        }
        // 先更新缓存
        if self.sidebar.side == side {
            self.sidebar.open = !self.sidebar.open;
        } else {
            self.sidebar.side = side;
            self.sidebar.open = true;
        }
        // 同步给 Sidebar entity
        if let Some(weak) = self.sidebar_entity.as_ref() {
            if let Some(mut entity) = weak.upgrade() {
                entity.update(cx, |s, cx| {
                    s.toggle(side, cx);
                });
            }
        }
        cx.notify();
    }

    pub fn set_side(&mut self, side: DockPosition, cx: &mut Context<Self>) {
        if side == DockPosition::Bottom {
            return;
        }
        self.sidebar.side = side;
        self.sidebar.open = true;
        if let Some(weak) = self.sidebar_entity.as_ref() {
            if let Some(mut entity) = weak.upgrade() {
                entity.update(cx, |s, cx| {
                    s.open(side, cx);
                });
            }
        }
        cx.notify();
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        div()
            .id("status-bar")
            .flex()
            .flex_row()
            .w_full()
            .items_center()
            .justify_between()
            .px_1()
            .py_1()
            .gap_2()
            .bg(colors.surface_background)
            .border_t_1()
            .border_color(colors.border_variant)
            .child(self.render_left_tools(cx))
            .child(self.render_right_tools(cx))
    }
}
