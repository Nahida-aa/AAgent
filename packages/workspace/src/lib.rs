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
pub mod workspace;
pub mod workspace_error;
mod workspace_settings;

pub use dock::{Panel, PanelEvent, PanelHandle, DockPosition};
pub use multi_workspace::{
    ProjectGroupKey,
    CloseWorkspaceSidebar, DraggedSidebar, FocusWorkspaceSidebar, MoveProjectDown,
    MoveProjectToNewWindow, MoveProjectUp, MultiWorkspace, MultiWorkspaceEvent, NewThread,
    NextProject, NextThread, PreviousProject, PreviousThread, ProjectGroup, RemovalIntent,
    SerializedProjectGroupState, Sidebar, SidebarEvent, SidebarHandle, SidebarRenderState,
    SidebarSide, ToggleWorkspaceSidebar, sidebar_side_context_menu,
};
pub use path_list::{PathList, SerializedPathList};
pub use remote::{
    RemoteConnectionIdentity, remote_connection_identity, same_remote_connection_identity,
};
pub use toast_layer::{ToastAction, ToastLayer, ToastView};

use anyhow::{Context as _, Result, anyhow};
use client::{
    ChannelId, Client, ErrorExt, ParticipantIndex, Status, TypedEnvelope, User, UserStore,
    proto::{self, ErrorCode, PanelId, PeerId},
};
use collections::{HashMap, HashSet, TypeIdHashMap, hash_map};
use dock::{Dock, PanelButtons, RESIZE_HANDLE_SIZE};
use fs::Fs;
use futures::{
    Future, FutureExt, StreamExt,
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{Shared, try_join_all},
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Action, AnyEntity, AnyView, AnyWeakView, App, AppContext, AsyncApp, AsyncWindowContext, Axis,
    Bounds, ClipboardItem, Context, CursorStyle, Decorations, DragMoveEvent, Entity, EntityId,
    EventEmitter, FocusHandle, Focusable, Global, HitboxBehavior, Hsla, KeyContext, Keystroke,
    ManagedView, MouseButton, PathPromptOptions, Point, PromptLevel, Render, ResizeEdge, Size,
    Stateful, Subscription, SystemWindowTabController, Task, TaskExt, Tiling, WeakEntity,
    WindowBounds, WindowHandle, WindowId, WindowOptions, actions, canvas, point, relative, size,
    transparent_black,
};
pub use history_manager::*;
pub use item::{
    FollowableItem, FollowableItemHandle, Item, ItemHandle, ItemSettings, PreviewTabsSettings,
    ProjectItem, SerializableItem, SerializableItemHandle, WeakItemHandle,
};
use itertools::Itertools;
use language::{Buffer, LanguageRegistry, Rope, language_settings::all_language_settings};
pub use modal_layer::*;
use node_runtime::NodeRuntime;
use notifications::{
    DetachAndPromptErr, Notifications, dismiss_app_notification,
    simple_message_notification::MessageNotification,
};
pub use pane::group::{
    ActivePaneDecorator, HANDLE_HITBOX_SIZE, Member, PaneAxis, PaneGroup, PaneRenderContext,
    SplitDirection,
};
pub use pane::*;
pub use persistence::{
    RecentWorkspace, WorkspaceDb, delete_unloaded_items,
    model::{
        DockData, DockStructure, ItemId, MultiWorkspaceState, SerializedMultiWorkspace,
        SerializedProjectGroup, SerializedWorkspaceLocation, SessionWorkspace,
    },
    read_serialized_multi_workspaces,
};
use persistence::{SerializedWindowBounds, model::SerializedWorkspace};
use postage::stream::Stream;
use project::{
    DirectoryLister, Project, ProjectEntryId, ProjectPath, ResolvedPath, Worktree, WorktreeId,
    WorktreeSettings,
    debugger::{breakpoint_store::BreakpointStoreEvent, session::ThreadStatus},
    git_store::{GitStoreEvent, RepositoryEvent},
    project_settings::ProjectSettings,
    toolchain_store::ToolchainStoreEvent,
    trusted_worktrees::{RemoteHostLocation, TrustedWorktrees, TrustedWorktreesEvent},
};
use release_channel::ReleaseChannel;
use remote::{
    RemoteClientDelegate, RemoteConnection, RemoteConnectionOptions,
    remote_client::ConnectionIdentifier,
};
use schemars::JsonSchema;
use serde::Deserialize;
use session::AppSession;
use settings::{
    CenteredPaddingSettings, DefaultOpenBehavior, Settings, SettingsLocation, SettingsStore,
    update_settings_file,
};

use aagent_actions::{Spawn, feedback::FileBugReport, theme::ToggleMode};
use sqlez::{
    bindable::{Bind, Column, StaticColumnCount},
    statement::Statement,
};
use status_bar::StatusBar;
use status_bar::add_hide_button_entry;
pub use status_bar::{HideStatusItem, StatusItemView};
use std::{
    any::TypeId,
    borrow::Cow,
    cell::{Cell, RefCell},
    cmp,
    collections::VecDeque,
    env,
    hash::Hash,
    path::{Path, PathBuf},
    process::ExitStatus,
    rc::Rc,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, AtomicUsize},
    },
    time::Duration,
};
use task::{DebugScenario, SharedTaskContext, SpawnInTerminal};
use theme::{ActiveTheme, SystemAppearance};
use theme_settings::ThemeSettings;
pub use toolbar::{
    PaneSearchBarCallbacks, Toolbar, ToolbarItemEvent, ToolbarItemLocation, ToolbarItemView,
};
pub use ui;
use ui::{Window, prelude::*};
use url::Url;
use util::{
    ResultExt, TryFutureExt,
    paths::{PathStyle, SanitizedPath},
    rel_path::RelPath,
    serde::default_true,
};
use uuid::Uuid;
pub use workspace_settings::{
    AccessibleMode, AutosaveSetting, BottomDockLayout, EncodingDisplayOptions, FocusFollowsMouse,
    RestoreOnStartupBehavior, StatusBarSettings, TabBarSettings, WorkspaceSettings,
    closing_last_window_quits_app, observe_accessible_mode,
};

use crate::{dock::PanelSizeState, item::ItemBufferKind, notifications::NotificationId};
use crate::{
    persistence::{
        SerializedAxis,
        model::{SerializedItem, SerializedPane, SerializedPaneGroup},
    },
    security_modal::SecurityModal,
};

// ========= workspace 嵌套模块的关键类型（crate 内部可见，顶级模块通过 crate::XXX 访问） =========
// Zed 单文件时这些类型直接定义在 workspace.rs 里，天然 crate 根可见。我们拆分到子模块后
// 需要这里 use 过来让 crate 根能看到。普通 use 足够了，不需要 pub use。
pub use crate::workspace::{
    actions::{
        ActivatePaneDown, ActivatePaneLeft, ActivatePaneRight, ActivatePaneUp, NewCenterTerminal,
        NewTerminal, SwapPaneAdjacent, SwapPaneDown, SwapPaneLeft, SwapPaneRight, SwapPaneUp,
        ToggleFileFinder, ToggleProjectSymbols, MovePaneDown, MovePaneLeft, MovePaneRight,
        MovePaneUp, OpenLog
    },
    app::state::{AppState, PreviousWorkspaceState},
    collab::{participant::{RemoteCollaborator, ParticipantLocation}, call::{GlobalAnyActiveCall, AnyActiveCall}, event::ActiveCallEvent},
    core::{
        WorkspaceId,
        actions::{
            ActivateNextPane, ActivatePreviousPane, NewFile, NewFileSplit, NewFileSplitHorizontal,
            NewFileSplitVertical, NewWindow, OpenInTerminal, OpenTerminal, ToggleReadOnlyFile,
            *,
        },
        event::Event,
        workspace::Workspace,
        lifecycle::reload
    },
    follow::{CollaboratorId, state::ViewId},
    notification::toast::Toast,
    open::{
        local::{open_new, with_active_or_new_workspace},
        options::{OpenOptions, OpenVisible, OpenMode,},
        remote::{open_remote_project_with_existing_connection, remote_workspace_position_from_db, open_remote_project_with_new_connection},
        matching::find_existing_workspace
    },

    pane::ops::move_active_item,
    registries::{
        FollowableViewRegistry, project_item::register_project_item,
        serializable_item::register_serializable_item,
    },
    providers::{TerminalProvider},
    serialize::SERIALIZATION_THROTTLE_TIME,
};
use workspace::{
    app::initial::init,
    app::store::WorkspaceStore,
    collab::room_project::join_in_room_project,
    core::actions::*,
    core::debounce::DelayedDebouncedEditAction,
    core::lifecycle::CloseIntent,
    core::lifecycle::prepare_window_to_close,
    dock::render::DraggedDock,
    follow::leader_border_for_pane,
    follow::{AutoWatch, FollowerState},
    item::permalink::{copy_file_permalink, open_file_permalink},
    open::local::{open_items, open_workspace_by_id},
    open::matching::{WorkspaceMatching, },
    open::options::{ OpenResult},
    open::prompt::{PromptForNewPath, PromptForOpenPath},
    open::windows::workspace_windows_for_location,
    pane::ActivateInDirectionTarget,
    providers::{DebuggerProvider, },
    registries::{ProjectItemRegistry, SerializableItemRegistry},
    serialize::WorkspaceLocation,
    serialize::flush::flush_windows_serialization,
    window::{client_side_decorations, title::WindowTitleContext},
};
// workspace 嵌套模块里定义，但需要 crate 根可见的类型
use workspace::follow::{follower::Follower, state::FollowerView};

use workspace::open::options::WorkspacePosition;
use workspace::pane::ops::move_item;
use workspace::registries::project_item::WorkspaceItemBuilder;
// dock constant
use dock::PANEL_SIZE_STATE_KEY;
