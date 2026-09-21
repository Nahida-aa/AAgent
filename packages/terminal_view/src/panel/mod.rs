//! TerminalPanel — 底部 Dock 里的终端面板。
//!
//! 对齐 Zed `crates/terminal_view/src/terminal_panel.rs`。
//! Zed 版持有完整 PaneGroup + 多 pane + 持久化 + action handler。
//! AAgent 最小版：一个 active_pane，每次 new_terminal() 创建 TerminalView 加进去。

pub mod inline_assist_tab_bar_button;
pub mod terminal_provider;

use aa_gpui_kit_ui::IconName;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, IntoElement, Render, WeakEntity, Window,
    actions, px,
};
use workspace::dock::panel::Panel;
use workspace::{DockPosition, Pane, PaneEvent, Workspace};

use crate::view::TerminalView;
mod actions;
mod failed_to_spawn;
mod helpers;
mod init;
mod pane_ops;
mod panel_impl;
mod render;
mod serialization;
mod tab_bar;
mod terminal_ops;
mod terminal_provider;

pub use actions::*;
pub use failed_to_spawn::*;
pub use helpers::*;
pub use init::*;
pub use terminal_provider::*;

use std::sync::Arc;
use std::time::Duration;

use collections::HashMap;
use gpui::{
    App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Task,
    WeakEntity, Window,
};
use project::{Fs, Project};
use settings::Settings;
use task::TaskId;
use terminal::Terminal;
use terminal::terminal_settings::TerminalSettings;
use workspace::{
    Pane, PaneGroup, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::TerminalView;

// ---------- TerminalPanel entity ----------

/// 底部终端面板 — 一个 Dock Panel，内部持有一个 Pane。
///
/// ```text
/// Dock (bottom)
///  └── TerminalPanel (render 这个)
///       └── active_pane: Entity<Pane>
///            └── items: Vec<Box<dyn ItemHandle>>  ← 每个都是 TerminalView
/// ```
pub struct TerminalPanel {
    pub(crate) active_pane: Entity<Pane>,
    pub(crate) center: PaneGroup,
    pub(super) focus_handle: FocusHandle,
    pub(super) fs: Arc<dyn Fs>,
    pub(super) workspace: WeakEntity<Workspace>,
    pub(super) pending_serialization: Task<Option<()>>,
    pub(super) pending_terminals_to_add: usize,
    pub(super) restoring: bool,
    pub(super) _restoration: Task<()>,
    pub(super) deferred_tasks: HashMap<TaskId, Task<()>>,
    pub(super) assistant_enabled: bool,
    /// 对齐 zed TerminalPanel::active — Dock 打开/关闭时由 Dock 调用 set_active 设置。
    /// true 时如果 pane 为空 → spawn 默认 terminal。
    pub(super) active: bool,
}

impl EventEmitter<PanelEvent> for TerminalPanel {}

impl Focusable for TerminalPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl TerminalPanel {
    pub fn new(workspace: &Workspace, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project = workspace.project();
        let pane =
            helpers::new_terminal_pane(workspace.weak_handle(), project.clone(), false, window, cx);
        let center = PaneGroup::new(pane.clone());
        let terminal_panel = Self {
            center,
            active_pane: pane,
            focus_handle: cx.focus_handle(),
            fs: workspace.app_state().fs.clone(),
            workspace: workspace.weak_handle(),
            pending_serialization: Task::ready(None),
            pending_terminals_to_add: 0,
            restoring: false,
            _restoration: Task::ready(()),
            deferred_tasks: HashMap::default(),
            assistant_enabled: false,
            active: false,
        };
        terminal_panel.apply_tab_bar_buttons(&terminal_panel.active_pane, cx);
        terminal_panel
    }

    pub fn assistant_enabled(&self) -> bool { self.assistant_enabled }

    /// Returns all panes in the terminal panel.
    pub fn panes(&self) -> Vec<&Entity<Pane>> { self.center.panes() }

    /// Returns all non-empty terminal selections from all terminal views in all panes.
    pub fn terminal_selections(&self, cx: &App) -> Vec<String> {
        self.center
            .panes()
            .iter()
            .flat_map(|pane| {
                pane.read(cx).items().filter_map(|item| {
                    let terminal_view = item.downcast::<crate::TerminalView>()?;
                    terminal_view
                        .read(cx)
                        .terminal()
                        .read(cx)
                        .last_content
                        .selection_text
                        .clone()
                        .filter(|text| !text.is_empty())
                })
            })
            .collect()
    }

    pub(super) fn is_enabled(&self, cx: &App) -> bool {
        self.workspace
            .upgrade()
            .is_some_and(|workspace| helpers::is_enabled_in_workspace(workspace.read(cx), cx))
    }

    pub(super) fn has_no_terminals(&self, cx: &App) -> bool {
        self.active_pane.read(cx).items_len() == 0 && self.pending_terminals_to_add == 0
    }
}
