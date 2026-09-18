//! 应用底部状态栏（status bar），结构对齐 zed `workspace::status_bar`。
//!
//! Zed 的状态栏是一排可增删的「项」（`StatusItemView`），左右两组
//! （`add_left_item` / `add_right_item`），右侧渲染时反序（先加的靠右、
//! 后加的靠左）。
//!
//! **Dock 面板按钮现在是独立的 PanelButtons entity**（见 [`crate::panel_buttons`]），
//! 关联一个 Dock entity，通过 StatusBar 的 `add_left_item`/`add_right_item`
//! 加入。StatusBar 本身不再持有 Dock 面板状态。
//!
//! - [`StatusItemView`]：item trait（对齐 zed）。
//! - [`StatusBar`]：容器，左侧/右侧各渲染一条 `h_flex`。
//! - **右键菜单**：普通项右键弹 `Hide`；Dock 面板按钮的右键菜单由 PanelButtons 自己处理。

pub mod items;

use std::any::TypeId;
use std::collections::HashSet;

use gpui::{AnyView, Context, Entity, ParentElement, Render, Styled, Window, prelude::*, px};
use ui_gpui::theme::ActiveTheme;
use ui_gpui::{
    ButtonRadius, ContextMenu, ContextMenuEntry, Divider, DividerColor, IconButton, IconName,
    Tooltip, right_click_menu,
};

use crate::dock_position::DockPosition;
use crate::panel_buttons::PanelButtons;

/// 状态栏项（对齐 zed `StatusItemView`）。
pub trait StatusItemView: Render {
    /// 自己有右键菜单（如 PanelButtons 的 Dock 切换），StatusBar 不要包 Hide。
    /// 默认 false → StatusBar 给它加 Hide Button。
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
        false // 无法 read，先返回 false；PanelButtons 用 TypeId 特化
    }
}

/// workspace 侧栏开关状态（对齐 zed `SidebarStatus{open, side}`）。
/// 注意：sidebar 和 dock 是 Zed 的两个独立概念——sidebar 是 MultiWorkspace 的
/// 导航栏（文件树），dock 是面板容器（Terminal/Agent 等）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SidebarStatus {
    pub open: bool,
    pub side: DockPosition,
}

/// 底部状态栏。只持有普通状态项 + sidebar 状态。
/// Dock 面板按钮由 PanelButtons entity（关联 Dock）通过 add_left_item/right_item 加入。
pub struct StatusBar {
    left_items: Vec<Box<dyn StatusItemViewHandle>>,
    right_items: Vec<Box<dyn StatusItemViewHandle>>,
    /// 被右键隐藏的普通项（对应 zed 的 hide_setting 隐藏）。
    hidden_items: HashSet<TypeId>,
    /// workspace 侧栏状态：单 sidebar，一行 state 定它是开是合、在左还是右。
    sidebar: SidebarStatus,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            left_items: Vec::new(),
            right_items: Vec::new(),
            hidden_items: HashSet::new(),
            sidebar: SidebarStatus {
                open: false,
                side: DockPosition::Left,
            },
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

    /// 移除某个具体类型的项（供 hide 使用）。
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

    /// 读取当前 sidebar 状态（Workspace Render 需要）。
    pub fn sidebar(&self) -> SidebarStatus {
        self.sidebar
    }

    /// 切换 workspace 侧栏的展开/折叠（对齐 zed `ToggleWorkspaceSidebar`）。
    /// 单 sidebar 模型：如果当前 sidebar 就在这侧则切换 open，
    /// 否则把 sidebar 移到这侧并展开。
    pub fn toggle_sidebar(&mut self, side: DockPosition) {
        if side == DockPosition::Bottom {
            return;
        }
        if self.sidebar.side == side {
            self.sidebar.open = !self.sidebar.open;
        } else {
            self.sidebar.side = side;
            self.sidebar.open = true;
        }
    }

    /// 设置 sidebar 在左侧还是右侧（sidebar toggle 右键菜单调用）。
    pub fn set_side(&mut self, side: DockPosition) {
        if side != DockPosition::Bottom {
            self.sidebar.side = side;
            self.sidebar.open = true;
        }
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

    /// 折叠时的 sidebar toggle（对齐 zed `StatusBar::render_sidebar_toggle`）。
    /// 右键菜单：sidebar 在 Left / Right 间切换（不能 Bottom，sidebar 是侧边栏概念）。
    fn render_sidebar_toggle(&self, on_right: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let bar = cx.entity();
        let current_side = self.sidebar.side;
        let divider = || Divider::vertical().color(DividerColor::Border);

        // Zed dock.rs:L242-251 — 根据 toggle 在左还是右选择菜单锚点
        let (menu_anchor, menu_attach) = if on_right {
            (gpui::Anchor::BottomRight, gpui::Anchor::TopRight)
        } else {
            (gpui::Anchor::BottomLeft, gpui::Anchor::TopLeft)
        };

        // Zed sidebar_side_context_menu — 只有 Left / Right 两项
        let bar_for_click = bar.clone();
        let bar_for_menu = bar.clone();
        let toggle = right_click_menu::<ContextMenu>("sidebar-toggle-menu")
            .anchor(menu_anchor)
            .attach(menu_attach)
            .trigger(move |_is_active, _window, _cx| {
                let bar = bar_for_click.clone();
                IconButton::new(
                    if on_right {
                        "toggle-workspace-sidebar"
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
                .aria_label("Open threads sidebar")
                // 状态栏按钮 tooltip 在上方弹出（状态栏在底部，默认向下会出屏）
                .tooltip(Tooltip::text("Open Threads Sidebar"))
                .tooltip_anchor(gpui::Anchor::BottomLeft)
                .tooltip_attach(gpui::Anchor::TopLeft)
                .on_click(move |_event, _window, cx| {
                    let side = if on_right {
                        DockPosition::Right
                    } else {
                        DockPosition::Left
                    };
                    bar.update(cx, |bar, _| bar.toggle_sidebar(side));
                })
                .into_any_element()
            })
            .menu(move |_window, cx| {
                let bar = bar_for_menu.clone();
                ContextMenu::build(cx, move |menu, _| {
                    let current = current_side;
                    let mut menu = menu;
                    let positions: [(DockPosition, &str); 2] =
                        [(DockPosition::Left, "Left"), (DockPosition::Right, "Right")];
                    for (pos, label) in positions {
                        let bar = bar.clone();
                        let is_current = pos == current;
                        menu =
                            menu.item(ContextMenuEntry::new(label).checked(is_current).on_click(
                                move |_window, cx| {
                                    bar.update(cx, |bar, _| bar.set_side(pos));
                                },
                            ));
                    }
                    menu
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
        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .children(children)
            .into_any_element()
    }

    /// 普通项 + 其右键 Hide 菜单。
    fn render_hideable(
        &self,
        ix: usize,
        item: &Box<dyn StatusItemViewHandle>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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

    /// 左组。先渲染左 sidebar toggle（折叠时），再所有 item。
    /// PanelButtons 跳过 Hide 包装（它自己有右键菜单）。
    fn render_left_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        if !self.sidebar.open && self.sidebar.side == DockPosition::Left {
            children.push(self.render_sidebar_toggle(false, cx).into_any_element());
        }
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

    /// 右组（渲染反转，先加的靠右）。末尾是右 sidebar toggle（折叠时）。
    fn render_right_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = Vec::new();
        for (ix, item) in self.visible_right_items().enumerate() {
            if item.item_type() == TypeId::of::<PanelButtons>() {
                children.push(item.to_any().into_any_element());
            } else {
                children.push(self.render_hideable(ix, item, cx).into_any_element());
            }
        }
        if !self.sidebar.open && self.sidebar.side == DockPosition::Right {
            children.push(self.render_sidebar_toggle(true, cx).into_any_element());
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
