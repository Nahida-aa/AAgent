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

use gpui::{
    Action, App, Axis, Bounds, Context, DragMoveEvent, Entity, IntoElement, ParentElement, Render,
    Styled, Window, canvas, div, hsla, prelude::*, px,
};
use theme::ActiveTheme;
use ui::StyledExt;

use dock::buttons::PanelButtons;
use dock::panel::{
    AgentPanel, CollabPanel, DebugPanel, GitPanel, OutlinePanel, Panel, PanelHandle, ProjectPanel,
};
use dock::{Dock, DraggedDock, RESIZE_HANDLE_SIZE};
use status_bar::StatusBar;

pub mod active_file_name;
pub mod history_manager;
pub mod invalid_item_view;
mod modal_layer;

pub mod notifications;
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

pub mod active_file_name;
pub mod dock;
pub mod history_manager;
pub mod invalid_item_view;
pub mod item;
mod modal_layer;
mod multi_workspace;

pub mod focus_follows_mouse;
pub mod notifications;
pub mod pane;
pub mod path_link;
mod persistence;
pub mod searchable;
pub mod security_modal;
pub mod shared_screen;
pub mod tasks;
mod theme_preview;
mod toast_layer;
mod toolbar;
pub mod welcome;
pub mod workspace_error;
mod workspace_settings;

pub mod path_list {
    pub use util::path_list::{PathList, SerializedPathList};
}

// 新增拆分
mod decorations;
mod registries;
mod shutdown;
mod startup;
mod types;

mod workspace;

// ---------- re-export：保持原有对外 API ----------

pub use dock::Panel;
pub use multi_workspace::{
    CloseWorkspaceSidebar, DraggedSidebar, FocusWorkspaceSidebar, MoveProjectDown,
    MoveProjectToNewWindow, MoveProjectUp, MultiWorkspace, MultiWorkspaceEvent, NewThread,
    NextProject, NextThread, PreviousProject, PreviousThread, ProjectGroup, ProjectGroupKey,
    RemovalIntent, SerializedProjectGroupState, Sidebar, SidebarEvent, SidebarHandle,
    SidebarRenderState, SidebarSide, ToggleWorkspaceSidebar, sidebar_side_context_menu,
};
pub use path_list::{PathList, SerializedPathList};
pub use remote::{
    RemoteConnectionIdentity, remote_connection_identity, same_remote_connection_identity,
};
pub use toast_layer::{ToastAction, ToastLayer, ToastView};

pub use history_manager::*;
pub use item::{
    FollowableItem, FollowableItemHandle, Item, ItemHandle, ItemSettings, PreviewTabsSettings,
    ProjectItem, SerializableItem, SerializableItemHandle, WeakItemHandle,
};
pub use modal_layer::*;
pub use pane::{
    group::{
        ActivePaneDecorator, HANDLE_HITBOX_SIZE, Member, PaneAxis, PaneGroup, PaneRenderContext,
        SplitDirection,
    },
    *,
};

pub use persistence::{
    RecentWorkspace, WorkspaceDb, delete_unloaded_items,
    model::{
        DockData, DockStructure, ItemId, MultiWorkspaceState, SerializedMultiWorkspace,
        SerializedProjectGroup, SerializedWorkspaceLocation, SessionWorkspace,
    },
    read_serialized_multi_workspaces,
};
pub use status_bar::{HideStatusItem, StatusItemView, add_hide_button_entry};
pub use toolbar::{
    PaneSearchBarCallbacks, Toolbar, ToolbarItemEvent, ToolbarItemLocation, ToolbarItemView,
};
pub use ui;
pub use workspace_settings::{
    AccessibleMode, AutosaveSetting, BottomDockLayout, EncodingDisplayOptions, FocusFollowsMouse,
    RestoreOnStartupBehavior, StatusBarSettings, TabBarSettings, WorkspaceSettings,
    closing_last_window_quits_app, observe_accessible_mode,
};

// 新拆分模块的 re-export
pub use decorations::{client_side_decorations, resize_edge};
pub use registries::{
    FollowableViewRegistry, ProjectItemRegistry, SerializableItemRegistry, register_project_item,
    register_serializable_item,
};
pub use shutdown::{prepare_window_to_close, prepare_windows_to_quit, reload};
pub use startup::*;
pub use types::{
    CloseIntent, OpenMode, OpenOptions, OpenResult, OpenVisible, WorkspaceId, WorkspaceLocation,
    WorkspaceMatching, WorkspacePosition,
};
pub use workspace::actions::*;
pub use workspace::active_call::{
    ActiveCallEvent, AnyActiveCall, GlobalAnyActiveCall, ParticipantLocation, RemoteCollaborator,
};
pub use workspace::app_state::{ActiveWorktreeCreation, AppState, PreviousWorkspaceState};
pub use workspace::collab::{FollowerState, ViewId, join_channel, join_in_room_project};
pub use workspace::permalink::{copy_file_permalink, open_file_permalink};
pub use workspace::terminal_providers::{DebuggerProvider, TerminalProvider};
pub use workspace::toast::Toast;
pub use workspace::window_title::{parse_window_title_format, render_window_title_format};
pub use workspace::workspace_store::{CollaboratorId, Follower};
pub use workspace::{AutoWatch, Workspace, WorkspaceHandle, WorkspaceStore};
