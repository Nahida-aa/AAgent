//! Workspace 层：对齐 zed `workspace` crate。
//!
//! AAgent 主线（GPUI 桌面）的整体布局 — 完全照搬 Zed 的 4 种 BottomDockLayout：
//!
//! ```
//! Contained (默认):           Full:
//! ┌──────┬───────┬──────┐    ┌───────────────────────────┐
//! │ Left │Center │Right │    │ Left  │   Center   │ Right │
//! │ Dock │Pane   │ Dock  │    │ Dock  │   Pane     │ Dock  │
//! │      ├───────┤       │    ├──────┴───┬───────┴──────┤
//! │      │Bottom │       │    │   Bottom Dock (全宽)    │
//! │      │ Dock  │       │    └───────────────────────────┘
//! └──────┴───────┴──────┘
//! ```
//!
//! 三层抽象（对齐 zed）：
//! - **Dock** — 面板容器（Left/Bottom/Right 三实例），内置 resize handle
//! - **PanelButtons** — 状态栏上的 Dock 按钮，关联一个 Dock entity
//! - **Workspace** — 顶层 entity，持有 3 Dock + center pane + status bar，
//!   顶层 div 监听 `on_drag_move<DraggedDock>` 接收所有 Dock resize 拖拽事件

pub mod dock;
pub mod item;
pub mod multi_workspace;
pub mod pane;
pub mod status_bar;
pub mod toolbar;

pub use item::{Item, ItemEvent, ItemHandle, WeakItemHandle};
pub use multi_workspace::{MultiWorkspace, SidebarHandle, SidebarRenderState};
pub use pane::group::{Member, PaneGroup};
pub use pane::{DraggedSelection, DraggedTab, Event as PaneEvent, Pane};
pub use settings_content::DockPosition;
pub use terminal::{NewCenterTerminal, NewTerminal, OpenTerminal, TerminalProvider};
pub use toolbar::{Toolbar, ToolbarItemEvent, ToolbarItemLocation, ToolbarItemView};

use std::collections::HashMap;
use std::sync::Arc;

use aa_gpui_kit_theme::ActiveTheme;
use aa_gpui_kit_ui::StyledExt;
use gpui::{
    Action, App, Axis, Bounds, Context, DragMoveEvent, Entity, IntoElement, ParentElement, Render,
    Styled, Window, canvas, div, hsla, prelude::*, px,
};

use dock::buttons::PanelButtons;
use dock::panel::{
    AgentPanel, CollabPanel, DebugPanel, GitPanel, OutlinePanel, Panel, PanelHandle, ProjectPanel,
};
use dock::{Dock, DraggedDock, RESIZE_HANDLE_SIZE};
use status_bar::StatusBar;

pub mod active_file_name;
pub mod dock;
pub mod history_manager;
pub mod invalid_item_view;
pub mod item;
mod modal_layer;
mod multi_workspace;

pub mod notifications;
pub mod pane;
pub mod path_list {
    pub use util::path_list::{PathList, SerializedPathList};
}
pub mod path_link;
mod persistence;
pub mod searchable;
pub mod security_modal;
pub mod shared_screen;
pub use shared_screen::SharedScreen;
pub mod focus_follows_mouse;
mod status_bar;
pub mod tasks;
mod theme_preview;
mod toast_layer;
mod toolbar;
pub mod welcome;
pub mod workspace_error;
mod workspace_settings;

mod workspace;
pub use workspace::*;

// 保留原来的 pub use dock::Panel; 等
pub use dock::Panel;
// pub use multi_workspace::{...};
pub use path_list::{PathList, SerializedPathList};
// pub use remote::{...};
pub use toast_layer::{ToastAction, ToastLayer, ToastView};

pub fn init(app_state: Arc<AppState>, cx: &mut App) {
    component::init();
    theme_preview::init(cx);
    toast_layer::init(cx);
    history_manager::init(app_state.fs.clone(), cx);

    cx.on_app_quit(flush_windows_serialization_on_quit).detach();

    cx.on_action(|_: &CloseWindow, cx| Workspace::close_global(cx))
        .on_action(|_: &Reload, cx| reload(cx))
        .on_action(|action: &Open, cx: &mut App| {
            let app_state = AppState::global(cx);
            prompt_and_open_paths(
                app_state,
                PathPromptOptions {
                    files: true,
                    directories: true,
                    multiple: true,
                    prompt: None,
                },
                action.create_new_window.unwrap_or_else(|| {
                    matches!(
                        WorkspaceSettings::get_global(cx).default_open_behavior,
                        DefaultOpenBehavior::NewWindow
                    )
                }),
                cx,
            );
        })
        .on_action(|_: &OpenFiles, cx: &mut App| {
            let directories = cx.can_select_mixed_files_and_dirs();
            let app_state = AppState::global(cx);
            prompt_and_open_paths(
                app_state,
                PathPromptOptions {
                    files: true,
                    directories,
                    multiple: true,
                    prompt: None,
                },
                true,
                cx,
            );
        });
}
