mod actions;
mod block;
mod context_menu;
mod events;
mod helpers;
mod hover;
mod ime;
mod input;
mod item;
mod lifecycle;
mod mode;
mod rename;
mod render;
mod scroll;
mod scrollbar_settings;
mod searchable;
mod serializable;
mod tab;
mod working_directory;

pub use actions::*;
pub use block::*;
pub use mode::*;

use crate::persistence::TerminalDb;
use crate::terminal_scrollbar::TerminalScrollHandle;
use editor::{Editor, blink_manager::BlinkManager};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Pixels, Subscription, Task, WeakEntity, Window,
};
use project::Project;
use std::rc::Rc;
use terminal::Terminal;
use workspace::{Workspace, WorkspaceId};

pub struct TerminalView {
    pub(super) terminal: Entity<Terminal>,
    pub(super) workspace: WeakEntity<Workspace>,
    pub(super) project: WeakEntity<Project>,
    pub(super) focus_handle: FocusHandle,
    pub(super) has_bell: bool,
    pub(super) context_menu: Option<(Entity<ContextMenu>, GpuiPoint<Pixels>, Subscription)>,
    pub(super) cursor_shape: CursorShape,
    pub(super) blink_manager: Entity<BlinkManager>,
    pub(super) mode: TerminalMode,
    pub(super) show_workspace_actions: Option<bool>,
    pub(super) blinking_terminal_enabled: bool,
    pub(super) needs_serialize: bool,
    pub(super) custom_title: Option<String>,
    pub(super) hover: Option<HoverTarget>,
    pub(super) hover_tooltip_update: Task<()>,
    pub(super) workspace_id: Option<WorkspaceId>,
    pub(super) show_breadcrumbs: bool,
    pub(super) block_below_cursor: Option<Rc<BlockProperties>>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_handle: TerminalScrollHandle,
    pub(super) ime_state: Option<ImeState>,
    pub(super) self_handle: WeakEntity<Self>,
    pub(super) rename_editor: Option<Entity<Editor>>,
    pub(super) rename_editor_subscription: Option<Subscription>,
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) _terminal_subscriptions: Vec<Subscription>,
}

impl EventEmitter<Event> for TerminalView {}
impl EventEmitter<ItemEvent> for TerminalView {}
impl EventEmitter<SearchEvent> for TerminalView {}
impl Focusable for TerminalView {
    /* 原样 */
}

impl TerminalView {
    pub fn new(
        terminal: Entity<Terminal>,
        workspace: WeakEntity<Workspace>,
        workspace_id: Option<WorkspaceId>,
        project: WeakEntity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workspace_handle = workspace.clone();
        let terminal_subscriptions =
            subscribe_for_terminal_events(&terminal, workspace, window, cx);

        let focus_handle = cx.focus_handle();
        let focus_in = cx.on_focus_in(&focus_handle, window, |terminal_view, window, cx| {
            terminal_view.focus_in(window, cx);
        });
        let focus_out = cx.on_focus_out(
            &focus_handle,
            window,
            |terminal_view, _event, window, cx| {
                terminal_view.focus_out(window, cx);
            },
        );
        let cursor_shape = TerminalSettings::get_global(cx).cursor_shape;

        let scroll_handle = TerminalScrollHandle::new(terminal.read(cx));

        let blink_manager = cx.new(|cx| {
            BlinkManager::new(
                CURSOR_BLINK_INTERVAL,
                |cx| {
                    !matches!(
                        TerminalSettings::get_global(cx).blinking,
                        TerminalBlink::Off
                    )
                },
                cx,
            )
        });

        let subscriptions = vec![
            focus_in,
            focus_out,
            cx.observe(&blink_manager, |_, _, cx| cx.notify()),
            cx.observe_global::<SettingsStore>(Self::settings_changed),
        ];

        Self {
            terminal,
            workspace: workspace_handle,
            project,
            has_bell: false,
            focus_handle,
            context_menu: None,
            cursor_shape,
            blink_manager,
            blinking_terminal_enabled: false,
            hover: None,
            hover_tooltip_update: Task::ready(()),
            mode: TerminalMode::Standalone,
            show_workspace_actions: None,
            workspace_id,
            show_breadcrumbs: TerminalSettings::get_global(cx).toolbar.breadcrumbs,
            block_below_cursor: None,
            scroll_top: Pixels::ZERO,
            scroll_handle,
            needs_serialize: false,
            custom_title: None,
            ime_state: None,
            self_handle: cx.entity().downgrade(),
            rename_editor: None,
            rename_editor_subscription: None,
            _subscriptions: subscriptions,
            _terminal_subscriptions: terminal_subscriptions,
        }
    }
    ///Create a new Terminal in the current working directory or the user's home directory
    pub fn deploy(
        workspace: &mut Workspace,
        action: &NewCenterTerminal,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let local = action.local;
        let working_directory = default_working_directory(workspace, cx);
        TerminalPanel::add_center_terminal(workspace, window, cx, move |project, cx| {
            if local {
                project.create_local_terminal(cx)
            } else {
                project.create_terminal_shell(working_directory, cx)
            }
        })
        .detach_and_log_err(cx);
    }
    pub fn entity(&self) -> &Entity<Terminal> { &self.terminal }
    pub fn terminal(&self) -> &Entity<Terminal> { &self.terminal }
    pub fn has_bell(&self) -> bool { self.has_bell }
}
