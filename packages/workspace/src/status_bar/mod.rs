//! 应用底部状态栏（status bar），结构对齐 zed `workspace::status_bar`。
//!
//! Zed 的状态栏是一排可增删的「项」（`StatusItemView`），左右两组
//! （`add_left_item` / `add_right_item`），右侧渲染时反序（先加的靠右、
//! 后加的靠左）。
//!
//! **Sidebar 状态现在由 MultiWorkspace + Sidebar entity 独立管理**
//! （见 [`crate::sidebar`]），StatusBar 不再持有 SidebarStatus、toggle_sidebar 等。
//! Sidebar toggle 按钮在 sidebar 关闭时由 MultiWorkspace 单独渲染。
//!
//! - [`StatusItemView`]：item trait（对齐 zed）。
//! - [`StatusBar`]：容器，左侧/右侧各渲染一条 `h_flex`。

pub mod items;

use std::any::TypeId;
use std::collections::HashSet;

use gpui::{AnyView, Context, Entity, ParentElement, Render, Styled, Window, prelude::*, px};
use ui_gpui::theme::ActiveTheme;
use ui_gpui::{ContextMenu, ContextMenuEntry};

use crate::dock::panel_buttons::PanelButtons;

/// 状态栏项（对齐 zed `StatusItemView`）。
pub trait StatusItemView: Render {
    /// 自己有右键菜单（如 PanelButtons 的 Dock 切换），StatusBar 不要包 Hide。
    fn has_custom_menu(&self) -> bool {
        false
    }
}

/// trait 对象化的 item handle（对齐 zed `StatusItemViewHandle`）。
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

/// 底部状态栏。只持有 Dock PanelButtons + 普通状态项。
/// Sidebar 状态由 MultiWorkspace / Sidebar entity 独立管理。
pub struct StatusBar {
    left_items: Vec<Box<dyn StatusItemViewHandle>>,
    right_items: Vec<Box<dyn StatusItemViewHandle>>,
    hidden_items: HashSet<TypeId>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            left_items: Vec::new(),
            right_items: Vec::new(),
            hidden_items: HashSet::new(),
        }
    }

    /// 向左组追加一个 item。Dock 的 PanelButtons 通过此方法加入。
    pub fn add_left_item<T: StatusItemView>(&mut self, item: Entity<T>) {
        self.left_items.push(Box::new(item));
    }

    /// 向右组追加一个 item；渲染时反序，所以**最后加的在最左**（zed 语义）。
    pub fn add_right_item<T: StatusItemView>(&mut self, item: Entity<T>) {
        self.right_items.push(Box::new(item));
    }

    /// 移除某个具体类型的项。
    pub fn remove_item_of_type<T: StatusItemView>(&mut self) {
        self.left_items
            .retain(|item| item.item_type() != TypeId::of::<T>());
        self.right_items
            .retain(|item| item.item_type() != TypeId::of::<T>());
    }

    /// 隐藏某个具体类型的普通项（普通项右键菜单动作）。
    pub fn hide_item(&mut self, item_type: TypeId) {
        self.hidden_items.insert(item_type);
    }

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

    /// 普通项 + 其右键 Hide 菜单。
    fn render_hideable(
        &self,
        ix: usize,
        item: &Box<dyn StatusItemViewHandle>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use ui_gpui::right_click_menu;
        let view = item.to_any();
        let item_type = item.item_type();
        let bar = cx.entity();
        right_click_menu::<ContextMenu>(("hideable-item", ix))
            .trigger(move |_is_active, _window, _cx| view.clone().into_any_element())
            .menu(move |_window, cx| {
                let bar = bar.clone();
                ContextMenu::build(cx, |menu, _| {
                    menu.item(ContextMenuEntry::new("Hide").on_click(move |_window, cx| {
                        bar.update(cx, |bar, _| bar.hide_item(item_type));
                    }))
                })
            })
    }

    /// 左组。PanelButtons 跳过 Hide 包装（它自己有右键菜单）。
    fn render_left_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        for (ix, item) in self.visible_left_items().enumerate() {
            if item.item_type() == TypeId::of::<PanelButtons>() {
                children.push(item.to_any().into_any_element());
            } else {
                children.push(self.render_hideable(ix, item, cx).into_any_element());
            }
        }
        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .min_w_0()
            .overflow_x_hidden()
            .children(children)
    }

    /// 右组（渲染反转，先加的靠右）。
    fn render_right_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        for (ix, item) in self.visible_right_items().enumerate() {
            if item.item_type() == TypeId::of::<PanelButtons>() {
                children.push(item.to_any().into_any_element());
            } else {
                children.push(self.render_hideable(ix, item, cx).into_any_element());
            }
        }
        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .children(children.into_iter().rev())
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        gpui::div()
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
