use collections::HashMap;
use gpui::{AppContext, Context, WeakEntity};

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use collections::HashMap as _;
use gpui::{
    AnyView, App, Bounds, Context, Entity, EntityId, EventEmitter, FocusHandle, Global, Pixels,
    Point, Subscription, Task, WeakEntity, Window,
};

pub mod app;
pub mod core;
pub mod serialize;
pub use crate::workspace::{app::initial::init, core::workspace::Workspace};

use crate::active_call::{ActiveCallEvent, AnyActiveCall, GlobalAnyActiveCall};
use crate::app_state::{ActiveWorktreeCreation, AppState, PreviousWorkspaceState};
use crate::collab::{FollowerState, ViewId};
use crate::dock::Dock;
use crate::item::{FollowableItemHandle, ItemHandle, WeakItemHandle};
use crate::modal_layer::ModalLayer;
use crate::multi_workspace::MultiWorkspace;
use crate::notifications::{NotificationId, Notifications};
use crate::pane::{Pane, SplitDirection};
use crate::pane_group::PaneGroup;
use crate::persistence::WorkspaceDb;
use crate::providers::{DebuggerProvider, TerminalProvider};
use crate::registries::{
    SerializableItemRegistry, register_project_item, register_serializable_item,
};
use crate::status_bar::StatusBar;
use crate::toast_layer::ToastLayer;

pub use crate::workspace::collab::AutoWatch;
pub use crate::workspace::store::WorkspaceStore;
use crate::{Pane, workspace::followers::CollaboratorId};

pub use core::{CloseIntent, Event, OpenMode, OpenVisible, Workspace, WorkspaceId};
pub use opening::{OpenOptions, OpenResult, WorkspaceMatching, open_paths, open_workspace_by_id};
pub use providers::{AnyActiveCall, DebuggerProvider, GlobalAnyActiveCall, TerminalProvider};
pub use registries::{register_project_item, register_serializable_item};
pub use window::title::{WindowTitleContext, WindowTitleNeeds};

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
