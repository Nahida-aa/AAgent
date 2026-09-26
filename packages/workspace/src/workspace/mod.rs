use super::*;
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

use collections::{HashMap, HashSet, TypeIdHashMap, hash_map};
use gpui::{
    Action, AnyEntity, AnyView, AnyWeakView, App, AppContext, AsyncApp, AsyncWindowContext, Axis,
    Bounds, ClipboardItem, Context, CursorStyle, Decorations, DragMoveEvent, Entity, EntityId,
    EventEmitter, FocusHandle, Focusable, Global, HitboxBehavior, Hsla, KeyContext, Keystroke,
    ManagedView, MouseButton, PathPromptOptions, Pixels, Point, PromptLevel, Render, ResizeEdge,
    Size, Stateful, Subscription, SystemWindowTabController, Task, TaskExt, Tiling, WeakEntity, Window,
    WindowBounds, WindowHandle, WindowId, WindowOptions, actions, canvas, point, relative, size,
    transparent_black,
};
use ui::prelude::*;
use futures::{
    Future, FutureExt, StreamExt,
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{Shared, try_join_all},
};

use anyhow::{Context as _, Result, anyhow};
use client::{
    ChannelId, Client, ErrorExt, ParticipantIndex, Status, TypedEnvelope, User, UserStore,
    proto::{self, ErrorCode, PanelId, PeerId},
};
use fs::Fs;
use itertools::Itertools;
use language::{Buffer, LanguageRegistry, Rope, language_settings::all_language_settings};
use node_runtime::NodeRuntime;
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
use sqlez::{
    bindable::{Bind, Column, StaticColumnCount},
    statement::Statement,
};
use task::{DebugScenario, SharedTaskContext, SpawnInTerminal};
use theme::{ActiveTheme, SystemAppearance};
use theme_settings::ThemeSettings;
use crate::workspace_settings::{WorkspaceSettings, BottomDockLayout};
use url::Url;
use util::{
    ResultExt, TryFutureExt,
    paths::{PathStyle, SanitizedPath},
    rel_path::RelPath,
    serde::default_true,
};
use uuid::Uuid;
use aagent_actions::{Spawn, feedback::FileBugReport, theme::ToggleMode};

pub mod app;
pub mod core;
pub mod serialize;
pub mod dock;
pub mod pane;
pub mod item;
pub mod collab;
pub mod follow;
pub mod modal;
pub mod nav;
pub mod notification;
pub use notification::toast::Toast;
pub mod open;
pub mod panel;
pub mod providers;
pub mod registries;
pub mod status_bar;
pub mod window;
pub mod worktree;
pub mod actions;
pub use actions::*;
pub use core::actions::*;
pub use crate::workspace::{app::initial::init, core::workspace::Workspace};

use crate::dock::{Dock, DockPosition, Panel, PanelButtons, PanelHandle, PanelSizeState};
use crate::item::{FollowableItemHandle, ItemBufferKind, ItemHandle, ProjectItem, SerializableItemHandle, WeakItemHandle};
use crate::modal_layer::ModalLayer;
use crate::multi_workspace::MultiWorkspace;
use crate::notifications::{NotificationId, Notifications};
use crate::pane::{Pane, SaveIntent, NavigationMode, SplitMode, group::{PaneGroup, SplitDirection, Member, AppState, FollowerState}};
use crate::persistence::{
    WorkspaceDb, SerializedAxis,
    model::{DockData, DockStructure, ItemId, MultiWorkspaceState, PathList, SerializedItem, SerializedMultiWorkspace, SerializedPane, SerializedPaneGroup, SerializedProjectGroupState, SerializedWorkspace, SerializedWorkspaceLocation},
};
use crate::status_bar::StatusBar;
use crate::multi_workspace::SidebarSide;
use crate::toast_layer::ToastLayer;
use crate::security_modal::SecurityModal;
use crate::multi_workspace::ProjectGroupKey;
use crate::notifications::simple_message_notification::MessageNotification;

// crate 根模块本身的导入（嵌套子模块通过 use super::* 继承后可直接写 persistence::xxx）
use crate::modal_layer;
use crate::notifications;
use crate::persistence;
use crate::workspace_error;

// std 常用模块（子模块通过 use super::* 继承）
use std::fmt;
use std::mem;

// workspace 内部子模块之间共享的类型（子模块通过 use super::* 继承）
pub(crate) use open::options::{OpenResult, OpenOptions};
pub(crate) use open::matching::{WorkspaceMatching, find_existing_workspace};
pub(crate) use open::windows::workspace_windows_for_location;
pub(crate) use open::remote::open_remote_project_with_existing_connection;
pub(crate) use collab::actions::OpenChannelNotes;
pub(crate) use collab::call::{AnyActiveCall, GlobalAnyActiveCall};
pub(crate) use collab::event::ActiveCallEvent;
pub(crate) use collab::participant::ParticipantLocation;
pub(crate) use collab::room_project::join_in_room_project;
pub(crate) use core::event::Event;
pub(crate) use app::state::{ActiveWorktreeCreation, PreviousWorkspaceState, AppState};
pub(crate) use core::lifecycle::{prepare_window_to_close, reload};
pub(crate) use window::{RegionFocusHandles, window_bounds_env_override};
pub(crate) use window::activation::activate_any_workspace_window;
pub(crate) use window::bounds::restore_native_window_state;
pub(crate) use window::title::format::render_window_title_format;
pub(crate) use follow::{ViewId, leader_border_for_pane, FollowerState, AutoWatch};
pub(crate) use pane::ActivateInDirectionTarget;
pub(crate) use pane::ops::{clone_active_item, move_active_item, join_pane_into_active, move_all_items};
pub(crate) use window::PartBehavior;
pub(crate) use serialize::flush::flush_windows_serialization;
pub(crate) use serialize::{WorkspaceLocation, SERIALIZATION_THROTTLE_TIME};
pub(crate) use open::local::{open_items, open_workspace_by_id};
pub(crate) use open::prompt::{prompt_and_open_paths, PromptForNewPath, PromptForOpenPath};
pub(crate) use dock::sizing::{px_with_ui_font_fallback, adjust_active_dock_size_by_px, adjust_open_docks_size_by_px};
pub(crate) use item::permalink::{open_file_permalink, copy_file_permalink};
pub(crate) use registries::{FollowableViewRegistry, ProjectItemRegistry, SerializableItemRegistry};
pub(crate) use notification::render::notify_if_database_failed;

pub use crate::workspace::app::store::WorkspaceStore;
use crate::workspace::follow::CollaboratorId;

use core::WorkspaceId;
use core::lifecycle::CloseIntent;
use open::options::{OpenMode, OpenVisible};
use window::title::WindowTitleContext;

/// Handles a workspace.
pub trait WorkspaceHandle {
    fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath>;
}

impl WorkspaceHandle for Entity<Workspace> {
    fn file_project_paths(&self, cx: &App) -> Vec<ProjectPath> {
        self.read(cx)
            .worktrees(cx)
            .flat_map(|worktree| {
                let worktree_id = worktree.read(cx).id();
                worktree.read(cx).files(true, 0).map(move |f| ProjectPath {
                    worktree_id,
                    path: f.path.clone(),
                })
            })
            .collect::<Vec<_>>()
    }
}
