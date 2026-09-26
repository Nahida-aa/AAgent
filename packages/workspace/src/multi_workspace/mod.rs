//! MultiWorkspace — 顶层窗口容器（对齐 zed `MultiWorkspace`）。
//!
//! Zed 的 MultiWorkspace 职责（multi_workspace.rs:L304-L322）：
//! - 管理多个 Workspace（窗口分屏）
//! - 持有 Sidebar dyn handle + sidebar_open/sidebar_side **状态**（自己存）
//! - Sidebar 独立于 Dock — 渲染顺序: [sidebar?] Workspace(main_area) [sidebar?]
//! - Sidebar 有 resize handle（对齐 zed L2110-L2160）

pub use sidebar::Sidebar;
pub use sidebar::handle::SidebarHandle;
pub use sidebar::render_state::SidebarRenderState;
pub use task::SpawnInTerminal;

// 外部 crate 导入（子模块通过 use super::* 继承）
use anyhow::{Context as _, Result};
use fs::Fs;
use gpui::{
    AnyView, App, Context, DragMoveEvent, Entity, EntityId, EventEmitter, FocusHandle,
    Focusable, IntoElement, ManagedView, MouseButton, ParentElement, Pixels, Render, Styled,
    Subscription, Task, TaskExt, WeakEntity, Window, WindowId, actions, deferred, div,
    prelude::*, px,
};
use project::{DisableAiSettings, Project};
pub(crate) use project::ProjectGroupKey;
use remote::RemoteConnectionOptions;
use agent_settings::AgentSettings;
use hold::HeldWorkspace;
use settings::{Settings, SidebarDockPosition};
use std::{cell::Cell, path::PathBuf, rc::Rc};
use theme::ActiveTheme;
use ui::{ContextMenu, right_click_menu, prelude::*};
use util::{ResultExt, path_list::PathList};

// crate 内部私有导入（子模块通过 use super::* 继承）
use crate::{
    CloseIntent, CloseWindow, DockPosition, Item, ModalView, OpenMode, Panel, WorkspaceId,
    persistence::model::MultiWorkspaceState,
};
use crate::Event as WorkspaceEvent;

// crate 内部转发（Workspace 是 crate 私有，用 pub(crate) 转发）
pub(crate) use crate::workspace::Workspace;
pub use settings_content::SidebarSide;
mod actions;
mod activate;
mod close;
mod delegate;
mod events;
mod find_workspace;
pub(crate) mod hold;
mod project_group;
mod project_group_ops;
mod remove;
mod render;
mod serialize;
mod sidebar;
mod sidebar_ops;

#[cfg(any(test, feature = "test-support"))]
mod test_helpers;

pub use actions::*;
pub use events::*;
pub use project_group::*;
pub use sidebar::*;

pub(crate) const SIDEBAR_RESIZE_HANDLE_SIZE: Pixels = px(6.0);
/// 顶层 MultiWorkspace entity。
///
/// Zed 对齐：
/// - multi_workspace.rs L316: `sidebar: Option<Box<dyn SidebarHandle>>` — dyn object
/// - multi_workspace.rs L317: `sidebar_open: bool` — **MultiWorkspace 自己存 open 状态**
/// - multi_workspace.rs L328: `sidebar_side()` — settings 层读 side
pub struct MultiWorkspace {
    pub(super) window_id: WindowId,
    pub(super) held: Vec<hold::HeldWorkspace>,
    pub(super) project_groups: Vec<ProjectGroupState>,
    /// Source of truth for which workspace is presented in this window, shared
    /// with each member `Workspace` so they can tell whether they own the
    /// platform window's title and edited indicator.
    pub(super) active_workspace_id: Rc<Cell<EntityId>>,
    pub(super) sidebar: Option<Box<dyn SidebarHandle>>,
    /// **MultiWorkspace 自己持有 sidebar open 状态**（对齐 zed L317）
    /// Sidebar entity 只是内容容器，不知道自己开没开
    pub(super) sidebar_open: bool,
    pub(super) sidebar_overlay: Option<AnyView>,
    pub(super) pending_removal_tasks: Vec<Task<()>>,
    pub(super) _serialize_task: Option<Task<()>>,
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) previous_focus_handle: Option<FocusHandle>,
}

impl EventEmitter<MultiWorkspaceEvent> for MultiWorkspace {}

impl MultiWorkspace {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let release_subscription = cx.on_release(|this: &mut MultiWorkspace, _cx| {
            if let Some(task) = this._serialize_task.take() {
                task.detach();
            }
            for task in std::mem::take(&mut this.pending_removal_tasks) {
                task.detach();
            }
        });
        let settings_subscription = cx.observe_global_in::<settings::SettingsStore>(window, {
            let mut previous_multi_workspace_enabled = !DisableAiSettings::get_global(cx)
                .disable_ai
                && AgentSettings::get_global(cx).enabled;
            move |this, window, cx| {
                let multi_workspace_enabled = this.multi_workspace_enabled(cx);
                if previous_multi_workspace_enabled && !multi_workspace_enabled {
                    this.collapse_to_single_workspace(window, cx);
                }
                previous_multi_workspace_enabled = multi_workspace_enabled;
            }
        });
        Self::subscribe_to_workspace(&workspace, window, cx);
        let weak_self = cx.weak_entity();
        let active_workspace_id = Rc::new(Cell::new(workspace.entity_id()));
        workspace.update(cx, |workspace, cx| {
            workspace.set_multi_workspace(weak_self, active_workspace_id.clone(), cx);
        });
        Self {
            window_id: window.window_handle().window_id(),
            held: vec![HeldWorkspace {
                workspace,
                pinned: false,
                activated_at: Some(0),
            }],
            project_groups: Vec::new(),
            active_workspace_id,
            sidebar: None,
            sidebar_open: false,
            sidebar_overlay: None,
            pending_removal_tasks: Vec::new(),
            _serialize_task: None,
            _subscriptions: vec![release_subscription, settings_subscription],
            previous_focus_handle: None,
        }
    }

    pub fn multi_workspace_enabled(&self, cx: &App) -> bool {
        !DisableAiSettings::get_global(cx).disable_ai && AgentSettings::get_global(cx).enabled
    }
}
