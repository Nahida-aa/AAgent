//! Pane — Tab 容器，持有多个 Item + 一个激活项。
//!
//! 对齐 zed `crates/workspace/src/pane.rs` 的核心子集。
//!
//! 子模块：
//! - [event] — Event enum（Empty, ActiveItemChanged, ItemAdded, ItemClosed）
//! - [history] — ActivationHistory，实现 "activate last"
//! - [navigation] — NavHistory，实现 "go back / go forward" 在 tab 间
//! - [activate_item] — ActivateItem action（带字段的 action 单独放）
//! - [dragged] — DraggedTab / DraggedSelection drag marker

// === 外部 crate 导入（子模块通过 use super::* 继承） ===
use gpui::{
    Action, Anchor, AnyElement, AnyView, App, AsyncWindowContext, ClickEvent, ClipboardItem,
    Context, Div, DragMoveEvent, Entity, EntityId, EventEmitter, ExternalPaths, FocusHandle,
    Focusable, IntoElement, KeyContext, MouseButton, NavigationDirection, Pixels, Point,
    PromptLevel, Render, ScrollHandle, Subscription, Task, TaskExt, WeakEntity, WeakFocusHandle,
    Window, actions, anchored, deferred, div, prelude::*, px,
};
use ui::{
    ButtonSize, ContextMenu, ContextMenuEntry, ContextMenuItem, DecoratedIcon, Headline,
    HeadlineSize, IconButton, IconButtonShape, IconDecoration, IconDecorationKind, IconName,
    IconSize, Indicator, PopoverMenu, PopoverMenuHandle, Tab, TabBar, TabPosition, Tooltip,
    prelude::*, right_click_menu,
};
use git::{CopyFilePermalink, OpenFilePermalink};
use itertools::Itertools;
use language::{Capability, DiagnosticSeverity};
use project::{DirectoryLister, Project, ProjectEntryId, ProjectPath, WorktreeId};
use collections::{BTreeSet, HashMap, HashSet, VecDeque};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{
    any::Any,
    cmp, fmt, mem,
    num::NonZeroUsize,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use parking_lot::Mutex;
use settings::{Settings, SettingsStore};
use theme_settings::ThemeSettings;
use util::{
    ResultExt, TryFutureExt, debug_panic, maybe, paths::PathStyle, serde::default_true,
};

// === crate 内部私有导入（子模块通过 use super::* 继承，不含 pub use 已有的类型避免 E0252） ===
use crate::{
    CloseWindow, NewCenterTerminal, NewFile, NewTerminal, OpenInTerminal, OpenOptions,
    OpenTerminal, OpenVisible, ToggleFileFinder, ToggleProjectSymbols, ToggleZoom,
    WorkspaceItemBuilder, ZoomIn, ZoomOut, focus_follows_mouse::FocusFollowsMouse as _,
    item::{ActivateOnClose, ClosePosition, SaveOptions, ShowCloseButton, ShowDiagnostics},
    move_item,
};

// === crate 内部转发（公开 API） ===
pub use crate::item::{
    Item, ItemBufferKind, ItemHandle, ItemSettings, PreviewTabsSettings, ProjectItemKind,
    TabContentParams, TabTooltipContent, WeakItemHandle,
};
pub use crate::{SplitDirection};
pub use crate::{
    invalid_item_view::InvalidItemView,
    toolbar::Toolbar,
    workspace_settings::{
        AutosaveSetting, FocusFollowsMouse, TabBarSettings, WorkspaceSettings,
    },
};

pub mod dragged;
pub mod event;
pub mod group;
pub use dragged::DraggedTab;
pub use event::Event;
mod actions;
mod add_item;
mod close;
mod focus;
mod helpers;
mod history;
mod mouse;
mod navigation;
mod pin;
mod preview;
mod queries;
mod render;
mod selection;
mod tab_bar;
mod zoom;

pub use actions::*;
pub use helpers::*;
pub use history::*;
pub use queries::*;
pub use selection::*;

pub struct ActivationHistoryEntry {
    pub entity_id: EntityId,
    pub timestamp: usize,
}

/// Tab 容器 — 装多个 Item，顶部 tab bar 切换。
pub struct Pane {
    pub(super) alternate_file_items: (
        Option<Box<dyn WeakItemHandle>>,
        Option<Box<dyn WeakItemHandle>>,
    ),
    pub(super) focus_handle: FocusHandle,
    pub(super) items: Vec<Box<dyn ItemHandle>>,
    pub(super) activation_history: Vec<ActivationHistoryEntry>,
    pub(super) next_activation_timestamp: Arc<AtomicUsize>,
    pub(super) zoomed: bool,
    pub(super) was_focused: bool,
    pub(super) active_item_index: usize,
    pub(super) preview_item_id: Option<EntityId>,
    pub(super) last_focus_handle_by_item: HashMap<EntityId, WeakFocusHandle>,
    pub(super) nav_history: NavHistory,
    pub(super) toolbar: Entity<Toolbar>,
    pub(crate) workspace: WeakEntity<Workspace>,
    pub(super) project: WeakEntity<Project>,
    pub drag_split_direction: Option<SplitDirection>,
    pub(super) can_drop_predicate: Option<Arc<dyn Fn(&dyn Any, &mut Window, &mut App) -> bool>>,
    pub(super) can_split_predicate:
        Option<Arc<dyn Fn(&mut Self, &dyn Any, &mut Window, &mut Context<Self>) -> bool>>,
    pub(super) can_toggle_zoom: bool,
    pub(super) should_display_tab_bar: Rc<dyn Fn(&Window, &mut Context<Pane>) -> bool>,
    pub(super) should_display_welcome_page: bool,
    pub(super) render_tab_bar_buttons: Rc<
        dyn Fn(
            &mut Pane,
            &mut Window,
            &mut Context<Pane>,
        ) -> (Option<AnyElement>, Option<AnyElement>),
    >,
    pub(super) render_tab_bar: Rc<dyn Fn(&mut Pane, &mut Window, &mut Context<Pane>) -> AnyElement>,
    pub(super) show_tab_bar_buttons: bool,
    pub(super) max_tabs: Option<NonZeroUsize>,
    pub(super) use_max_tabs: bool,
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) tab_bar_scroll_handle: ScrollHandle,
    pub(super) suppress_scroll: bool,
    pub(super) display_nav_history_buttons: Option<bool>,
    pub(super) double_click_dispatch_action: Box<dyn Action>,
    pub(super) save_modals_spawned: HashSet<EntityId>,
    pub(super) close_pane_if_empty: bool,
    pub new_item_context_menu_handle: PopoverMenuHandle<ContextMenu>,
    pub split_item_context_menu_handle: PopoverMenuHandle<ContextMenu>,
    pub(super) pinned_tab_count: usize,
    pub(super) diagnostics: HashMap<ProjectPath, DiagnosticSeverity>,
    pub(super) zoom_out_on_close: bool,
    pub(super) focus_follows_mouse: FocusFollowsMouse,
    pub(super) diagnostic_summary_update: Task<()>,
    pub project_item_restoration_data: HashMap<ProjectItemKind, Box<dyn Any + Send>>,
    pub(super) welcome_page: Option<Entity<crate::welcome::WelcomePage>>,
    pub in_center_group: bool,
}

pub enum Side {
    Left,
    Right,
}

#[derive(Copy, Clone)]
pub(super) enum PinOperation {
    Pin,
    Unpin,
}

impl EventEmitter<Event> for Pane {}

impl Focusable for Pane {
    fn focus_handle(&self, _cx: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl Pane {
    pub fn new(
        workspace: WeakEntity<Workspace>,
        project: Entity<Project>,
        next_timestamp: Arc<AtomicUsize>,
        can_drop_predicate: Option<Arc<dyn Fn(&dyn Any, &mut Window, &mut App) -> bool + 'static>>,
        double_click_dispatch_action: Box<dyn Action>,
        use_max_tabs: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let max_tabs = if use_max_tabs {
            WorkspaceSettings::get_global(cx).max_tabs
        } else {
            None
        };

        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, Pane::focus_in),
            cx.on_focus_in(&focus_handle, window, Pane::focus_in),
            cx.on_focus_out(&focus_handle, window, Pane::focus_out),
            cx.observe_global_in::<SettingsStore>(window, Self::settings_changed),
            cx.subscribe(&project, Self::project_events),
        ];

        let handle = cx.entity().downgrade();

        Self {
            alternate_file_items: (None, None),
            focus_handle,
            items: Vec::new(),
            activation_history: Vec::new(),
            next_activation_timestamp: next_timestamp.clone(),
            was_focused: false,
            zoomed: false,
            active_item_index: 0,
            preview_item_id: None,
            max_tabs,
            use_max_tabs,
            last_focus_handle_by_item: Default::default(),
            nav_history: NavHistory(Arc::new(Mutex::new(NavHistoryState {
                mode: NavigationMode::Normal,
                backward_stack: Default::default(),
                forward_stack: Default::default(),
                closed_stack: Default::default(),
                tag_stack: Default::default(),
                tag_stack_pos: Default::default(),
                paths_by_item: Default::default(),
                pane: handle,
                next_timestamp,
                preview_item_id: None,
            }))),
            toolbar: cx.new(|_| Toolbar::new()),
            tab_bar_scroll_handle: ScrollHandle::new(),
            suppress_scroll: false,
            drag_split_direction: None,
            workspace,
            project: project.downgrade(),
            can_drop_predicate,
            can_split_predicate: None,
            can_toggle_zoom: true,
            should_display_tab_bar: Rc::new(|_, cx| TabBarSettings::get_global(cx).show),
            should_display_welcome_page: false,
            render_tab_bar_buttons: Rc::new(default_render_tab_bar_buttons),
            render_tab_bar: Rc::new(Self::render_tab_bar),
            show_tab_bar_buttons: TabBarSettings::get_global(cx).show_tab_bar_buttons,
            display_nav_history_buttons: Some(
                TabBarSettings::get_global(cx).show_nav_history_buttons,
            ),
            _subscriptions: subscriptions,
            double_click_dispatch_action,
            save_modals_spawned: HashSet::default(),
            close_pane_if_empty: true,
            split_item_context_menu_handle: Default::default(),
            new_item_context_menu_handle: Default::default(),
            pinned_tab_count: 0,
            diagnostics: Default::default(),
            zoom_out_on_close: true,
            focus_follows_mouse: WorkspaceSettings::get_global(cx).focus_follows_mouse,
            diagnostic_summary_update: Task::ready(()),
            project_item_restoration_data: HashMap::default(),
            welcome_page: None,
            in_center_group: false,
        }
    }

    // 设置方法，全部保持 pub
    pub fn set_should_display_tab_bar<F>(&mut self, should_display_tab_bar: F)
    where
        F: 'static + Fn(&Window, &mut Context<Pane>) -> bool,
    {
        self.should_display_tab_bar = Rc::new(should_display_tab_bar);
    }

    pub fn set_should_display_welcome_page(&mut self, should_display_welcome_page: bool) {
        self.should_display_welcome_page = should_display_welcome_page;
    }

    pub fn set_can_split(
        &mut self,
        can_split_predicate: Option<
            Arc<dyn Fn(&mut Self, &dyn Any, &mut Window, &mut Context<Self>) -> bool + 'static>,
        >,
    ) {
        self.can_split_predicate = can_split_predicate;
    }

    pub fn set_can_toggle_zoom(&mut self, can_toggle_zoom: bool, cx: &mut Context<Self>) {
        self.can_toggle_zoom = can_toggle_zoom;
        cx.notify();
    }

    pub fn set_close_pane_if_empty(&mut self, close_pane_if_empty: bool, cx: &mut Context<Self>) {
        self.close_pane_if_empty = close_pane_if_empty;
        cx.notify();
    }

    pub fn set_can_navigate(&mut self, can_navigate: bool, cx: &mut Context<Self>) {
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_can_navigate(can_navigate, cx);
        });
        cx.notify();
    }

    pub fn set_render_tab_bar<F>(&mut self, cx: &mut Context<Self>, render: F)
    where
        F: 'static + Fn(&mut Pane, &mut Window, &mut Context<Pane>) -> AnyElement,
    {
        self.render_tab_bar = Rc::new(render);
        cx.notify();
    }

    pub fn set_render_tab_bar_buttons<F>(&mut self, cx: &mut Context<Self>, render: F)
    where
        F: 'static
            + Fn(
                &mut Pane,
                &mut Window,
                &mut Context<Pane>,
            ) -> (Option<AnyElement>, Option<AnyElement>),
    {
        self.render_tab_bar_buttons = Rc::new(render);
        cx.notify();
    }

    pub fn nav_history_for_item<T: Item>(&self, item: &Entity<T>) -> ItemNavHistory {
        ItemNavHistory {
            history: self.nav_history.clone(),
            item: Arc::new(item.downgrade()),
        }
    }

    pub fn nav_history(&self) -> &NavHistory { &self.nav_history }

    pub fn nav_history_mut(&mut self) -> &mut NavHistory { &mut self.nav_history }

    pub fn fork_nav_history(&self) -> NavHistory {
        let history = self.nav_history.0.lock().clone();
        NavHistory(Arc::new(Mutex::new(history)))
    }

    pub fn set_nav_history(&mut self, history: NavHistory, cx: &Context<Self>) {
        self.nav_history = history;
        self.nav_history().0.lock().pane = cx.entity().downgrade();
    }

    pub fn disable_history(&mut self) { self.nav_history.disable(); }

    pub fn enable_history(&mut self) { self.nav_history.enable(); }

    pub fn can_navigate_backward(&self) -> bool {
        !self.nav_history.0.lock().backward_stack.is_empty()
    }

    pub fn can_navigate_forward(&self) -> bool {
        !self.nav_history.0.lock().forward_stack.is_empty()
    }
    pub fn display_nav_history_buttons(&mut self, display: Option<bool>) {
        self.display_nav_history_buttons = display;
    }
    pub fn set_zoom_out_on_close(&mut self, zoom_out_on_close: bool) {
        self.zoom_out_on_close = zoom_out_on_close;
    }
}
