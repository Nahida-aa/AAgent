use super::*;

use anyhow::Context as _;
use client::proto;
use db::kvp::KeyValueStore;
use gpui::{
    Action, App, AppContext, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Render,
    StyleRefinement, Styled, Subscription, WeakEntity, Window, deferred, div, px,
};
use settings::{Settings, SettingsStore};
use std::sync::Arc;

use crate::Workspace;
use crate::focus_follows_mouse::FocusFollowsMouse as _;
use crate::persistence::model::DockData;
use crate::{DraggedDock, Event, FocusFollowsMouse, ModalLayer, Pane, WorkspaceSettings};

use super::panel::{Panel, PanelEvent, PanelHandle};
use super::position::DockPosition;
use super::size::{PanelEntry, PanelSizeState, panel_uses_flexible_width, resize_panel_entry};
use super::{PANEL_SIZE_STATE_KEY, RESIZE_HANDLE_SIZE};

pub(crate) enum DockRestoreState {
    Restoring { pending: Option<DockData> },
    Finished,
}

impl DockRestoreState {
    fn pending(&self) -> Option<&DockData> {
        match self {
            Self::Restoring { pending } => pending.as_ref(),
            Self::Finished => None,
        }
    }

    fn discard_pending(&mut self) {
        if let Self::Restoring { pending } = self {
            *pending = None;
        }
    }
}

pub struct Dock {
    pub(super) position: DockPosition,
    pub(super) panel_entries: Vec<PanelEntry>,
    pub(super) workspace: WeakEntity<Workspace>,
    pub(super) is_open: bool,
    pub(super) active_panel_index: Option<usize>,
    pub(super) focus_handle: FocusHandle,
    focus_follows_mouse: FocusFollowsMouse,
    restoration: DockRestoreState,
    zoom_layer_open: bool,
    modal_layer: Entity<ModalLayer>,
    _subscriptions: [Subscription; 2],
}

impl Focusable for Dock {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl Dock {
    pub fn new(
        position: DockPosition,
        modal_layer: Entity<ModalLayer>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let focus_handle = cx.focus_handle();
        let workspace = cx.entity();
        let dock = cx.new(|cx| {
            let focus_subscription =
                cx.on_focus(&focus_handle, window, |dock: &mut Dock, window, cx| {
                    if let Some(active_entry) = dock.active_panel_entry() {
                        active_entry
                            .panel
                            .activation_focus_handle(cx)
                            .focus(window, cx)
                    }
                });
            let zoom_subscription = cx.subscribe(&workspace, |dock, workspace, e: &Event, cx| {
                if matches!(e, Event::ZoomChanged) {
                    let is_zoomed = workspace.read(cx).zoomed.is_some();
                    dock.zoom_layer_open = is_zoomed;
                }
            });
            Self {
                position,
                workspace: workspace.downgrade(),
                panel_entries: Default::default(),
                active_panel_index: None,
                is_open: false,
                focus_handle: focus_handle.clone(),
                focus_follows_mouse: WorkspaceSettings::get_global(cx).focus_follows_mouse,
                _subscriptions: [focus_subscription, zoom_subscription],
                restoration: DockRestoreState::Restoring { pending: None },
                zoom_layer_open: false,
                modal_layer,
            }
        });

        cx.on_focus_in(&focus_handle, window, {
            let dock = dock.downgrade();
            move |workspace, window, cx| {
                let Some(dock) = dock.upgrade() else {
                    return;
                };
                let Some(panel) = dock.read(cx).active_panel() else {
                    return;
                };
                if panel.is_zoomed(window, cx) {
                    workspace.zoomed = Some(panel.to_any().downgrade());
                    workspace.zoomed_position = Some(position);
                } else {
                    workspace.zoomed = None;
                    workspace.zoomed_position = None;
                }
                cx.emit(Event::ZoomChanged);
                workspace.dismiss_zoomed_items_to_reveal(Some(position), window, cx);
                workspace.update_active_view_for_followers(window, cx)
            }
        })
        .detach();

        cx.observe_in(&dock, window, move |workspace, dock, window, cx| {
            if dock.read(cx).is_open()
                && let Some(panel) = dock.read(cx).active_panel()
                && panel.is_zoomed(window, cx)
            {
                workspace.zoomed = Some(panel.to_any().downgrade());
                workspace.zoomed_position = Some(position);
                cx.emit(Event::ZoomChanged);
                return;
            }
            if workspace.zoomed_position == Some(position) {
                workspace.zoomed = None;
                workspace.zoomed_position = None;
                cx.emit(Event::ZoomChanged);
            }
        })
        .detach();

        dock
    }
    pub fn position(&self) -> DockPosition { self.position }
    pub fn is_open(&self) -> bool { self.is_open }
    fn resizable(&self, cx: &App) -> bool {
        !(self.zoom_layer_open || self.modal_layer.read(cx).has_active_modal())
    }

    pub fn panel<T: Panel>(&self) -> Option<Entity<T>> {
        self.panel_entries
            .iter()
            .find_map(|entry| entry.panel.to_any().downcast().ok())
    }

    pub fn panel_index_for_type<T: Panel>(&self) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.to_any().downcast::<T>().is_ok())
    }

    pub fn panel_index_for_persistent_name(&self, ui_name: &str, _cx: &App) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.persistent_name() == ui_name)
    }
    pub fn panel_index_for_proto_id(&self, panel_id: PanelId) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.remote_id() == Some(panel_id))
    }

    pub fn panel_for_id(&self, panel_id: EntityId) -> Option<&Arc<dyn PanelHandle>> {
        self.panel_entries
            .iter()
            .find(|entry| entry.panel.panel_id() == panel_id)
            .map(|entry| &entry.panel)
    }

    pub fn first_enabled_panel_idx(&mut self, cx: &mut Context<Self>) -> anyhow::Result<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.enabled(cx))
            .with_context(|| {
                format!(
                    "Couldn't find any enabled panel for the {} dock.",
                    self.position.label()
                )
            })
    }

    fn active_panel_entry(&self) -> Option<&PanelEntry> {
        self.active_panel_index
            .and_then(|index| self.panel_entries.get(index))
    }

    fn active_panel_entry_mut(&mut self) -> Option<&mut PanelEntry> {
        self.active_panel_index
            .and_then(|index| self.panel_entries.get_mut(index))
    }
    pub fn active_panel_index(&self) -> Option<usize> { self.active_panel_index }

    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open != self.is_open {
            self.restoration.discard_pending();
        }

        cx.notify();
    }

    pub fn set_panel_zoomed(
        &mut self,
        panel: &gpui::AnyView,
        zoomed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for entry in &mut self.panel_entries {
            if entry.panel.panel_id() == panel.entity_id() {
                if zoomed != entry.panel.is_zoomed(window, cx) {
                    entry.panel.set_zoomed(zoomed, window, cx);
                }
            } else if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }

        self.workspace
            .update(cx, |workspace, cx| {
                workspace.serialize_workspace(window, cx);
            })
            .ok();

        cx.notify();
    }
    pub fn zoom_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for entry in &mut self.panel_entries {
            if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }
    }

    pub(crate) fn add_panel<T: Panel>(
        &mut self,
        panel: Entity<T>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let subscriptions = [
            cx.observe(&panel, |_, _, cx| cx.notify()),
            cx.observe_global_in::<SettingsStore>(window, {
                let workspace = workspace.clone();
                let panel = panel.clone();

                move |this, window, cx| {
                    let new_position = panel.read(cx).position(window, cx);
                    if new_position == this.position {
                        return;
                    }

                    let Ok(new_dock) = workspace.update(cx, |workspace, cx| {
                        if panel.is_zoomed(window, cx) {
                            workspace.zoomed_position = Some(new_position);
                        }
                        match new_position {
                            DockPosition::Left => &workspace.left_dock,
                            DockPosition::Bottom => &workspace.bottom_dock,
                            DockPosition::Right => &workspace.right_dock,
                        }
                        .clone()
                    }) else {
                        return;
                    };

                    let panel_id = Entity::entity_id(&panel);
                    let was_visible = this.is_open()
                        && this
                            .visible_panel()
                            .is_some_and(|active_panel| active_panel.panel_id() == panel_id);
                    let size_state = this
                        .panel_entries
                        .iter()
                        .find(|entry| entry.panel.panel_id() == panel_id)
                        .map(|entry| entry.size_state)
                        .unwrap_or_default();

                    let previous_axis = this.position.axis();
                    let next_axis = new_position.axis();
                    let size_state = if previous_axis == next_axis {
                        size_state
                    } else {
                        PanelSizeState::default()
                    };

                    if !this.remove_panel(&panel, window, cx) {
                        // Panel was already moved from this dock
                        return;
                    }

                    new_dock.update(cx, |new_dock, cx| {
                        let index =
                            new_dock.add_panel(panel.clone(), workspace.clone(), window, cx);
                        if let Some(added_panel) = new_dock.panel_for_id(panel_id).cloned() {
                            new_dock.set_panel_size_state(added_panel.as_ref(), size_state, cx);
                        }
                        if was_visible {
                            new_dock.set_open(true, window, cx);
                            new_dock.activate_panel(index, window, cx);
                        }
                    });

                    workspace
                        .update(cx, |workspace, cx| {
                            workspace.serialize_workspace(window, cx);
                        })
                        .ok();
                }
            }),
            {
                let panel = panel.clone();
                let mut last_default_size = panel.read(cx).default_size(window, cx);

                cx.observe_global_in::<SettingsStore>(window, move |this, window, cx| {
                    let default_size = panel.read(cx).default_size(window, cx);
                    if default_size == last_default_size {
                        return;
                    }
                    last_default_size = default_size;

                    let panel_id = Entity::entity_id(&panel);
                    if let Some(entry) = this
                        .panel_entries
                        .iter_mut()
                        .find(|entry| entry.panel.panel_id() == panel_id)
                    {
                        entry.size_state.size = None;
                        entry.panel.size_state_changed(window, cx);
                        cx.notify();
                    }
                })
            },
            cx.subscribe_in(
                &panel,
                window,
                move |this, panel, event, window, cx| match event {
                    PanelEvent::ZoomIn => {
                        this.set_panel_zoomed(&panel.to_any(), true, window, cx);
                        if !PanelHandle::panel_focus_handle(panel, cx).contains_focused(window, cx)
                        {
                            window.focus(&panel.read(cx).activation_focus_handle(cx), cx);
                        }
                        workspace
                            .update(cx, |workspace, cx| {
                                workspace.zoomed = Some(panel.downgrade().into());
                                workspace.zoomed_position =
                                    Some(panel.read(cx).position(window, cx));
                                cx.emit(Event::ZoomChanged);
                            })
                            .ok();
                    }
                    PanelEvent::ZoomOut => {
                        this.set_panel_zoomed(&panel.to_any(), false, window, cx);
                        workspace
                            .update(cx, |workspace, cx| {
                                if workspace.zoomed_position == Some(this.position) {
                                    workspace.zoomed = None;
                                    workspace.zoomed_position = None;
                                    cx.emit(Event::ZoomChanged);
                                }
                                cx.notify();
                            })
                            .ok();
                    }
                    PanelEvent::Activate => {
                        if let Some(ix) = this
                            .panel_entries
                            .iter()
                            .position(|entry| entry.panel.panel_id() == Entity::entity_id(panel))
                        {
                            this.set_open(true, window, cx);
                            this.activate_panel(ix, window, cx);
                            window.focus(&panel.read(cx).activation_focus_handle(cx), cx);
                        }
                    }
                    PanelEvent::Close => {
                        if this
                            .visible_panel()
                            .is_some_and(|p| p.panel_id() == Entity::entity_id(panel))
                        {
                            this.set_open(false, window, cx);
                        }
                    }
                },
            ),
        ];

        let index = match self
            .panel_entries
            .binary_search_by_key(&panel.read(cx).activation_priority(), |entry| {
                entry.panel.activation_priority(cx)
            }) {
            Ok(ix) => {
                if cfg!(debug_assertions) {
                    panic!(
                        "Panels `{}` and `{}` have the same activation priority. Each panel must have a unique priority so the status bar order is deterministic.",
                        T::panel_key(),
                        self.panel_entries[ix].panel.panel_key()
                    );
                }
                ix
            }
            Err(ix) => ix,
        };
        if let Some(active_index) = self.active_panel_index.as_mut()
            && *active_index >= index
        {
            *active_index += 1;
        }
        let size_state = panel.read(cx).initial_size_state(window, cx);

        self.panel_entries.insert(
            index,
            PanelEntry {
                panel: Arc::new(panel.clone()),
                size_state,
                _subscriptions: subscriptions,
            },
        );

        self.replay_pending_serialized_state(window, cx);

        if panel.read(cx).starts_open(window, cx) {
            self.activate_panel_internal(index, window, cx);
            self.set_open_internal(true, window, cx);
        }

        cx.notify();
        index
    }

    pub(crate) fn restore_serialized_state(
        &mut self,
        serialized: DockData,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &mut self.restoration {
            DockRestoreState::Restoring { pending } => {
                *pending = Some(serialized);
                self.replay_pending_serialized_state(window, cx);
            }
            DockRestoreState::Finished => {
                let active_panel_missing = serialized
                    .active_panel
                    .as_deref()
                    .filter(|_| serialized.visible)
                    .is_some_and(|name| self.panel_index_for_persistent_name(name, cx).is_none());
                if !active_panel_missing {
                    self.apply_serialized_state(&serialized, window, cx);
                }
            }
        }
    }
    fn replay_pending_serialized_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(serialized) = self.restoration.pending().cloned() else {
            return;
        };
        let waiting_for_active_panel = serialized
            .active_panel
            .as_deref()
            .filter(|_| serialized.visible)
            .is_some_and(|name| self.panel_index_for_persistent_name(name, cx).is_none());
        if waiting_for_active_panel {
            self.set_open_internal(serialized.visible, window, cx);
        } else {
            self.apply_serialized_state(&serialized, window, cx);
            self.restoration.discard_pending();
        }
    }
    fn apply_serialized_state(
        &mut self,
        serialized: &DockData,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(active_panel) = serialized
            .active_panel
            .as_deref()
            .filter(|_| serialized.visible)
            && let Some(idx) = self.panel_index_for_persistent_name(active_panel, cx)
        {
            self.activate_panel_internal(idx, window, cx);
        }
        if serialized.zoom
            && let Some(panel) = self.active_panel()
        {
            panel.set_zoomed(true, window, cx)
        }
        self.set_open_internal(serialized.visible, window, cx);
    }
    pub(crate) fn finish_restoration(&mut self) { self.restoration = DockRestoreState::Finished; }

    pub fn remove_panel<T: Panel>(
        &mut self,
        panel: &Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(panel_ix) = self
            .panel_entries
            .iter()
            .position(|entry| entry.panel.panel_id() == Entity::entity_id(panel))
        {
            if let Some(active_panel_index) = self.active_panel_index.as_mut() {
                match panel_ix.cmp(active_panel_index) {
                    std::cmp::Ordering::Less => {
                        *active_panel_index -= 1;
                    }
                    std::cmp::Ordering::Equal => {
                        self.active_panel_index = None;
                        self.set_open(false, window, cx);
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }

            self.panel_entries.remove(panel_ix);
            cx.notify();

            true
        } else {
            false
        }
    }

    pub fn panels_len(&self) -> usize { self.panel_entries.len() }
    pub fn has_agent_panel(&self, cx: &App) -> bool {
        self.panel_entries
            .iter()
            .any(|entry| entry.panel.is_agent_panel(cx))
    }

    pub fn activate_panel(&mut self, panel_ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if Some(panel_ix) != self.active_panel_index {
            self.restoration.discard_pending();
        }
        self.activate_panel_internal(panel_ix, window, cx);
    }
    fn activate_panel_internal(
        &mut self,
        panel_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if Some(panel_ix) != self.active_panel_index {
            if let Some(active_panel) = self.active_panel_entry() {
                active_panel.panel.set_active(false, window, cx);
            }

            self.active_panel_index = Some(panel_ix);
            if let Some(active_panel) = self.active_panel_entry() {
                active_panel.panel.set_active(true, window, cx);
            }

            cx.notify();
        }
    }

    pub fn visible_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        let entry = self.visible_entry()?;
        Some(&entry.panel)
    }
    pub fn active_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        let panel_entry = self.active_panel_entry()?;
        Some(&panel_entry.panel)
    }
    fn visible_entry(&self) -> Option<&PanelEntry> {
        if self.is_open {
            self.active_panel_entry()
        } else {
            None
        }
    }
    pub fn zoomed_panel(&self, window: &Window, cx: &App) -> Option<Arc<dyn PanelHandle>> {
        let entry = self.visible_entry()?;
        if entry.panel.is_zoomed(window, cx) {
            Some(entry.panel.clone())
        } else {
            None
        }
    }
    pub fn active_panel_size(&self) -> Option<PanelSizeState> {
        if self.is_open {
            self.active_panel_entry().map(|entry| entry.size_state)
        } else {
            None
        }
    }

    pub fn stored_panel_size(
        &self,
        panel: &dyn PanelHandle,
        window: &Window,
        cx: &App,
    ) -> Option<Pixels> {
        self.panel_entries
            .iter()
            .find(|entry| entry.panel.panel_id() == panel.panel_id())
            .map(|entry| {
                entry
                    .size_state
                    .size
                    .unwrap_or_else(|| entry.panel.default_size(window, cx))
            })
    }
    pub fn stored_panel_size_state(&self, panel: &dyn PanelHandle) -> Option<PanelSizeState> {
        self.panel_entries
            .iter()
            .find(|entry| entry.panel.panel_id() == panel.panel_id())
            .map(|entry| entry.size_state)
    }
    pub fn stored_active_panel_size(&self, window: &Window, cx: &App) -> Option<Pixels> {
        if self.is_open {
            self.active_panel_entry().map(|entry| {
                entry
                    .size_state
                    .size
                    .unwrap_or_else(|| entry.panel.default_size(window, cx))
            })
        } else {
            None
        }
    }
    pub fn set_panel_size_state(
        &mut self,
        panel: &dyn PanelHandle,
        size_state: PanelSizeState,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(entry) = self
            .panel_entries
            .iter_mut()
            .find(|entry| entry.panel.panel_id() == panel.panel_id())
        {
            entry.size_state = size_state;
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn toggle_panel_flexible_size(
        &mut self,
        panel: &dyn PanelHandle,
        current_size: Option<Pixels>,
        current_flex: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .panel_entries
            .iter_mut()
            .find(|entry| entry.panel.panel_id() == panel.panel_id())
        else {
            return;
        };
        let currently_flexible = entry.panel.has_flexible_size(window, cx);
        if currently_flexible {
            entry.size_state.size = current_size;
        } else {
            entry.size_state.flex = current_flex;
        }
        let panel_key = entry.panel.panel_key();
        let size_state = entry.size_state;
        let workspace = self.workspace.clone();
        entry
            .panel
            .set_flexible_size(!currently_flexible, window, cx);
        entry.panel.size_state_changed(window, cx);
        cx.defer(move |cx| {
            if let Some(workspace) = workspace.upgrade() {
                workspace.update(cx, |workspace, cx| {
                    workspace.persist_panel_size_state(panel_key, size_state, cx);
                });
            }
        });
        cx.notify();
    }

    fn resize_active_panel(
        &mut self,
        size: Option<Pixels>,
        flex: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = self.position;
        if let Some(entry) = self.active_panel_entry_mut() {
            let (panel_key, size_state) =
                resize_panel_entry(position, entry, size, flex, window, cx);

            let workspace = self.workspace.clone();
            cx.defer(move |cx| {
                if let Some(workspace) = workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        workspace.persist_panel_size_state(panel_key, size_state, cx);
                    });
                }
            });
            cx.notify();
        }
    }
    pub fn resize_panel_sizes(
        &mut self,
        size: Option<Pixels>,
        flex: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = self.position;
        if let Some(entry) = self.active_panel_entry_mut() {
            let (panel_key, size_state) =
                resize_panel_entry(position, entry, size, flex, window, cx);

            let workspace = self.workspace.clone();
            cx.defer(move |cx| {
                if let Some(workspace) = workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        workspace.persist_panel_size_state(panel_key, size_state, cx);
                    });
                }
            });
            cx.notify();
        }
    }
    pub fn reset_panel_sizes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.should_resize_all_panels(cx) {
            self.resize_all_panels(size, flex, window, cx);
        } else {
            self.resize_active_panel(size, flex, window, cx);
        }
    }
    fn reset_active_panel_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = PanelSizeState::default();
        self.resize_active_panel(state.size, state.flex, window, cx);
    }
    fn reset_all_panel_sizes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_entry) = self.active_panel_entry() else {
            return;
        };
        let size =
            (!panel_uses_flexible_width(self.position, active_entry.panel.as_ref(), window, cx))
                .then(|| active_entry.panel.default_size(window, cx));
        self.resize_all_panels(size, None, window, cx);
    }
    fn should_resize_all_panels(&self, cx: &App) -> bool {
        WorkspaceSettings::get_global(cx)
            .resize_all_panels_in_dock
            .contains(&self.position)
    }
    fn resize_all_panels(
        &mut self,
        size: Option<Pixels>,
        flex: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_panel_index) = self.active_panel_index else {
            return;
        };

        let active_panel_uses_flexible_width = {
            let Some(active_entry) = self.panel_entries.get(active_panel_index) else {
                return;
            };
            panel_uses_flexible_width(self.position, active_entry.panel.as_ref(), window, cx)
        };
        let mut size_states_to_persist = Vec::new();
        for entry in &mut self.panel_entries {
            if panel_uses_flexible_width(self.position, entry.panel.as_ref(), window, cx)
                == active_panel_uses_flexible_width
            {
                size_states_to_persist.push(resize_panel_entry(
                    self.position,
                    entry,
                    size,
                    flex,
                    window,
                    cx,
                ));
            }
        }

        let workspace = self.workspace.clone();
        cx.defer(move |cx| {
            if let Some(workspace) = workspace.upgrade() {
                workspace.update(cx, |workspace, cx| {
                    for (panel_key, size_state) in size_states_to_persist {
                        workspace.persist_panel_size_state(panel_key, size_state, cx);
                    }
                });
            }
        });

        cx.notify();
    }

    pub fn toggle_action(&self) -> Box<dyn Action> {
        match self.position {
            DockPosition::Left => crate::workspace::ToggleLeftDock.boxed_clone(),
            DockPosition::Bottom => crate::workspace::ToggleBottomDock.boxed_clone(),
            DockPosition::Right => crate::workspace::ToggleRightDock.boxed_clone(),
        }
    }
    fn dispatch_context() -> gpui::KeyContext {
        let mut dispatch_context = KeyContext::new_with_defaults();
        dispatch_context.add("Dock");

        dispatch_context
    }

    pub fn clamp_panel_size(&mut self, max_size: Pixels, window: &Window, cx: &mut Context<Self>) {
        let max_size = (max_size - RESIZE_HANDLE_SIZE).abs();
        let mut clamped = false;
        for entry in &mut self.panel_entries {
            let uses_flexible_width =
                panel_uses_flexible_width(self.position, entry.panel.as_ref(), window, cx);
            if uses_flexible_width {
                continue;
            }

            let size = entry
                .size_state
                .size
                .unwrap_or_else(|| entry.panel.default_size(window, cx));
            if size > max_size {
                entry.size_state.size = Some(max_size.max(RESIZE_HANDLE_SIZE));
                clamped = true;
            }
        }
        if clamped {
            cx.notify();
        }
    }

    pub(crate) fn load_persisted_size_state(
        workspace: &Workspace,
        panel_key: &'static str,
        cx: &App,
    ) -> Option<PanelSizeState> {
        let workspace_id = workspace
            .database_id()
            .map(|id| i64::from(id).to_string())
            .or(workspace.session_id())?;
        let kvp = KeyValueStore::global(cx);
        let scope = kvp.scoped(PANEL_SIZE_STATE_KEY);
        scope
            .read(&format!("{workspace_id}:{panel_key}"))
            .log_err()
            .flatten()
            .and_then(|json| serde_json::from_str::<PanelSizeState>(&json).log_err())
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dispatch_context = Self::dispatch_context();
        if let Some(entry) = self.visible_entry() {
            let position = self.position;
            let create_resize_handle = || {
                let handle = div()
                    .id("resize-handle")
                    .on_drag(DraggedDock(position), |dock, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| dock.clone())
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|dock, e: &MouseUpEvent, window, cx| {
                            if e.click_count == 2 {
                                dock.reset_panel_sizes(window, cx);
                                dock.workspace
                                    .update(cx, |workspace, cx| {
                                        workspace.serialize_workspace(window, cx);
                                    })
                                    .ok();
                                cx.stop_propagation();
                            }
                        }),
                    )
                    .occlude();
                match self.position() {
                    DockPosition::Left => deferred(
                        handle
                            .absolute()
                            .right(-RESIZE_HANDLE_SIZE / 2.)
                            .top(px(0.))
                            .h_full()
                            .w(RESIZE_HANDLE_SIZE)
                            .cursor_col_resize(),
                    ),
                    DockPosition::Bottom => deferred(
                        handle
                            .absolute()
                            .top(-RESIZE_HANDLE_SIZE / 2.)
                            .left(px(0.))
                            .w_full()
                            .h(RESIZE_HANDLE_SIZE)
                            .cursor_row_resize(),
                    ),
                    DockPosition::Right => deferred(
                        handle
                            .absolute()
                            .top(px(0.))
                            .left(-RESIZE_HANDLE_SIZE / 2.)
                            .h_full()
                            .w(RESIZE_HANDLE_SIZE)
                            .cursor_col_resize(),
                    ),
                }
            };

            div()
                .id("dock-panel")
                .key_context(dispatch_context)
                .track_focus(&self.focus_handle(cx))
                .focus_follows_mouse(self.focus_follows_mouse, cx)
                .flex()
                .bg(cx.theme().colors().panel_background)
                .border_color(cx.theme().colors().border)
                .overflow_hidden()
                .map(|this| match self.position().axis() {
                    // Width and height are always set on the workspace wrapper in
                    // render_dock, so fill whatever space the wrapper provides.
                    Axis::Horizontal => this.w_full().h_full().flex_row(),
                    Axis::Vertical => this.h_full().w_full().flex_col(),
                })
                .map(|this| match self.position() {
                    DockPosition::Left => this.border_r_1(),
                    DockPosition::Right => this.border_l_1(),
                    DockPosition::Bottom => this.border_t_1(),
                })
                .child(
                    div()
                        .map(|this| match self.position().axis() {
                            Axis::Horizontal => this.w_full().h_full(),
                            Axis::Vertical => this.h_full().w_full(),
                        })
                        .child(
                            entry
                                .panel
                                .to_any()
                                .cached(StyleRefinement::default().v_flex().size_full()),
                        ),
                )
                .when(self.resizable(cx), |this| {
                    this.child(create_resize_handle())
                })
        } else {
            div()
                .id("dock-panel")
                .key_context(dispatch_context)
                .track_focus(&self.focus_handle(cx))
        }
    }
}
