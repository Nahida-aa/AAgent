use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use client::{Client, proto};
use futures::channel::mpsc;
use gpui::{
    Action, AnyElement, AnyEntity, AnyView, App, AppContext, Context, Entity, EntityId,
    FocusHandle, Focusable, Font, Pixels, Point, SharedString, Task, TaskExt, Window,
};
use language::{Capability, HighlightedText};
use project::{Project, ProjectEntryId, ProjectPath};
use settings::SettingsLocation;
use smallvec::SmallVec;
use ui::Icon;
use util::ResultExt;

use super::events::{ItemBufferKind, ItemEvent, SaveOptions, TabContentParams, TabTooltipContent};
use super::follow::{FollowEvent, FollowableItemHandle};
use super::handle::ItemHandle;
use super::serializable::SerializableItemHandle;
use super::traits::Item;
use super::weak_handle::WeakItemHandle;
use crate::item::LEADER_UPDATE_THROTTLE;
use crate::searchable::SearchableItemHandle;
use crate::toolbar::ToolbarItemLocation;
use crate::workspace_settings::{AutosaveSetting, WorkspaceSettings};
use crate::{
    CollaboratorId, DelayedDebouncedEditAction, FollowableViewRegistry, ItemNavHistory,
    SerializableItemRegistry, Workspace, WorkspaceId,
    pane::{self, Pane},
};

impl<T: Item> ItemHandle for Entity<T> {
    fn subscribe_to_item_events(
        &self,
        window: &mut Window,
        cx: &mut App,
        handler: Box<dyn Fn(ItemEvent, &mut Window, &mut App)>,
    ) -> gpui::Subscription {
        window.subscribe(self, cx, move |_, event, window, cx| {
            T::to_item_events(event, &mut |item_event| handler(item_event, window, cx));
        })
    }

    fn item_focus_handle(&self, cx: &App) -> FocusHandle { self.read(cx).focus_handle(cx) }

    fn telemetry_event_text(&self, cx: &App) -> Option<&'static str> {
        self.read(cx).telemetry_event_text()
    }

    fn tab_content(&self, params: TabContentParams, window: &Window, cx: &App) -> AnyElement {
        self.read(cx).tab_content(params, window, cx)
    }
    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString {
        self.read(cx).tab_content_text(detail, cx)
    }

    fn suggested_filename(&self, cx: &App) -> SharedString { self.read(cx).suggested_filename(cx) }

    fn tab_icon(&self, window: &Window, cx: &App) -> Option<Icon> {
        self.read(cx).tab_icon(window, cx)
    }

    fn tab_tooltip_content(&self, cx: &App) -> Option<TabTooltipContent> {
        self.read(cx).tab_tooltip_content(cx)
    }

    fn tab_tooltip_text(&self, cx: &App) -> Option<SharedString> {
        self.read(cx).tab_tooltip_text(cx)
    }

    fn dragged_tab_content(
        &self,
        params: TabContentParams,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        self.read(cx).tab_content(
            TabContentParams {
                selected: true,
                ..params
            },
            window,
            cx,
        )
    }

    fn project_path(&self, cx: &App) -> Option<ProjectPath> {
        <T as Item>::active_project_path(self.read(cx), cx)
    }

    fn workspace_settings<'a>(&self, cx: &'a App) -> &'a WorkspaceSettings {
        if let Some(project_path) = self.project_path(cx) {
            WorkspaceSettings::get(
                Some(SettingsLocation {
                    worktree_id: project_path.worktree_id,
                    path: &project_path.path,
                }),
                cx,
            )
        } else {
            WorkspaceSettings::get_global(cx)
        }
    }

    fn project_entry_ids(&self, cx: &App) -> SmallVec<[ProjectEntryId; 3]> {
        let mut result = SmallVec::new();
        self.read(cx).for_each_project_item(cx, &mut |_, item| {
            if let Some(id) = item.entry_id(cx) {
                result.push(id);
            }
        });
        result
    }

    fn project_paths(&self, cx: &App) -> SmallVec<[ProjectPath; 3]> {
        let mut result = SmallVec::new();
        self.read(cx).for_each_project_item(cx, &mut |_, item| {
            if let Some(id) = item.project_path(cx) {
                result.push(id);
            }
        });
        result
    }

    fn project_item_model_ids(&self, cx: &App) -> SmallVec<[EntityId; 3]> {
        let mut result = SmallVec::new();
        self.read(cx).for_each_project_item(cx, &mut |id, _| {
            result.push(id);
        });
        result
    }

    fn for_each_project_item(
        &self,
        cx: &App,
        f: &mut dyn FnMut(EntityId, &dyn project::ProjectItem),
    ) {
        self.read(cx).for_each_project_item(cx, f)
    }

    fn buffer_kind(&self, cx: &App) -> ItemBufferKind { self.read(cx).buffer_kind(cx) }

    fn boxed_clone(&self) -> Box<dyn ItemHandle> { Box::new(self.clone()) }

    fn can_split(&self, cx: &App) -> bool { self.read(cx).can_split() }

    fn clone_on_split(
        &self,
        workspace_id: Option<WorkspaceId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Box<dyn ItemHandle>>> {
        let task = self.update(cx, |item, cx| item.clone_on_split(workspace_id, window, cx));
        cx.background_spawn(async move {
            task.await
                .map(|handle| Box::new(handle) as Box<dyn ItemHandle>)
        })
    }

    fn added_to_pane(
        &self,
        workspace: &mut Workspace,
        pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let weak_item = self.downgrade();
        let history = pane.read(cx).nav_history_for_item(self);
        self.update(cx, |this, cx| {
            this.set_nav_history(history, window, cx);
            this.added_to_workspace(workspace, window, cx);
        });

        if let Some(serializable_item) = self.to_serializable_item_handle(cx) {
            workspace
                .enqueue_item_serialization(serializable_item)
                .log_err();
        }

        let new_pane_id = pane.entity_id();
        let old_item_pane = workspace
            .panes_by_item
            .insert(self.item_id(), pane.downgrade());

        if old_item_pane.as_ref().is_none_or(|old_pane| {
            old_pane
                .upgrade()
                .is_some_and(|old_pane| old_pane.entity_id() != new_pane_id)
        }) {
            self.update(cx, |this, cx| {
                this.pane_changed(new_pane_id, cx);
            });
        }

        if old_item_pane.is_none() {
            let mut pending_autosave = DelayedDebouncedEditAction::new();
            let (pending_update_tx, mut pending_update_rx) = mpsc::unbounded();
            let pending_update = Rc::new(RefCell::new(None));

            let mut send_follower_updates = None;
            if let Some(item) = self.to_followable_item_handle(cx) {
                let is_project_item = item.is_project_item(window, cx);
                let item = item.downgrade();

                send_follower_updates = Some(cx.spawn_in(window, {
                    let pending_update = pending_update.clone();
                    async move |workspace, cx| {
                        while let Ok(mut leader_id) = pending_update_rx.recv().await {
                            while let Ok(id) = pending_update_rx.try_recv() {
                                leader_id = id;
                            }

                            workspace.update_in(cx, |workspace, window, cx| {
                                let Some(item) = item.upgrade() else { return };
                                workspace.update_followers(
                                    is_project_item,
                                    proto::update_followers::Variant::UpdateView(
                                        proto::UpdateView {
                                            id: item
                                                .remote_id(workspace.client(), window, cx)
                                                .and_then(|id| id.to_proto()),
                                            variant: pending_update.borrow_mut().take(),
                                            leader_id,
                                        },
                                    ),
                                    window,
                                    cx,
                                );
                            })?;
                            cx.background_executor().timer(LEADER_UPDATE_THROTTLE).await;
                        }
                        anyhow::Ok(())
                    }
                }));
            }

            let mut event_subscription = Some(cx.subscribe_in(
                self,
                window,
                move |workspace, item: &Entity<T>, event, window, cx| {
                    let pane = if let Some(pane) = workspace
                        .panes_by_item
                        .get(&item.item_id())
                        .and_then(|pane| pane.upgrade())
                    {
                        pane
                    } else {
                        return;
                    };

                    if let Some(item) = item.to_followable_item_handle(cx) {
                        let leader_id = workspace.leader_for_pane(&pane);

                        if let Some(leader_id) = leader_id
                            && let Some(FollowEvent::Unfollow) = item.to_follow_event(event)
                        {
                            workspace.unfollow(leader_id, window, cx);
                        }

                        if item.item_focus_handle(cx).contains_focused(window, cx) {
                            match leader_id {
                                Some(CollaboratorId::Agent) => {}
                                Some(CollaboratorId::PeerId(leader_peer_id)) => {
                                    item.add_event_to_update_proto(
                                        event,
                                        &mut pending_update.borrow_mut(),
                                        window,
                                        cx,
                                    );
                                    pending_update_tx.unbounded_send(Some(leader_peer_id)).ok();
                                }
                                None => {
                                    item.add_event_to_update_proto(
                                        event,
                                        &mut pending_update.borrow_mut(),
                                        window,
                                        cx,
                                    );
                                    pending_update_tx.unbounded_send(None).ok();
                                }
                            }
                        }
                    }

                    if let Some(item) = item.to_serializable_item_handle(cx)
                        && item.should_serialize(event, cx)
                    {
                        workspace.enqueue_item_serialization(item).ok();
                    }

                    T::to_item_events(event, &mut |event| match event {
                        ItemEvent::CloseItem => {
                            pane.update(cx, |pane, cx| {
                                pane.close_item_by_id(
                                    item.item_id(),
                                    crate::SaveIntent::Close,
                                    window,
                                    cx,
                                )
                            })
                            .detach_and_log_err(cx);
                        }

                        ItemEvent::UpdateTab => {
                            workspace.update_item_dirty_state(item, window, cx);

                            if item.has_deleted_file(cx)
                                && !item.is_dirty(cx)
                                && item.workspace_settings(cx).close_on_file_delete
                            {
                                let item_id = item.item_id();
                                let close_item_task = pane.update(cx, |pane, cx| {
                                    pane.close_item_by_id(
                                        item_id,
                                        crate::SaveIntent::Close,
                                        window,
                                        cx,
                                    )
                                });
                                cx.spawn_in(window, {
                                    let pane = pane.clone();
                                    async move |_workspace, cx| {
                                        close_item_task.await?;
                                        pane.update(cx, |pane, _cx| {
                                            pane.nav_history_mut().remove_item(item_id);
                                        });
                                        anyhow::Ok(())
                                    }
                                })
                                .detach_and_log_err(cx);
                            } else {
                                pane.update(cx, |_, cx| {
                                    cx.emit(pane::Event::ChangeItemTitle);
                                    cx.notify();
                                });
                            }
                        }

                        ItemEvent::UpdateBreadcrumbs => {
                            if &pane == workspace.active_pane()
                                && pane.read(cx).active_item().is_some_and(|active_item| {
                                    active_item.item_id() == item.item_id()
                                })
                            {
                                workspace.active_item_path_changed(false, window, cx);
                            }
                        }

                        ItemEvent::Edit => {
                            let autosave = item.workspace_settings(cx).autosave;

                            if let AutosaveSetting::AfterDelay { milliseconds } = autosave {
                                let delay = Duration::from_millis(milliseconds.0);
                                let item = item.clone();
                                pending_autosave.fire_new(
                                    delay,
                                    window,
                                    cx,
                                    move |workspace, window, cx| {
                                        Pane::autosave_item(
                                            &item,
                                            workspace.project().clone(),
                                            window,
                                            cx,
                                        )
                                    },
                                );
                            }
                            pane.update(cx, |pane, cx| pane.handle_item_edit(item.item_id(), cx));
                        }
                    });
                },
            ));

            cx.on_focus_out(
                &self.read(cx).focus_handle(cx),
                window,
                move |workspace, _event, window, cx| {
                    if let Some(item) = weak_item.upgrade()
                        && item.workspace_settings(cx).autosave == AutosaveSetting::OnFocusChange
                    {
                        // Only trigger autosave if focus has truly left the item.
                        // If focus is still within the item's hierarchy (e.g., moved to a context menu),
                        // don't trigger autosave to avoid unwanted formatting and cursor jumps.
                        let focus_handle = item.item_focus_handle(cx);
                        if focus_handle.contains_focused(window, cx) {
                            return;
                        }

                        // Add the item to a deferred save list. The actual save will happen when
                        // focus lands on a pane or panel (via handle_pane_focused or
                        // handle_panel_focused), or when the window deactivates.
                        // This avoids saving when opening modals and skips saving if focus
                        // returns to the same item.
                        workspace.deferred_save_items.push(item.downgrade_item());

                        // Defer the flush to ensure all focus events are processed first.
                        // This is needed because on_focus_out fires before handle_pane_focused
                        // when switching items.
                        cx.defer_in(window, |workspace, window, cx| {
                            // Don't flush if a modal is active - the user might return
                            // to the original item when the modal is dismissed.
                            if !workspace.has_active_modal(window, cx) {
                                workspace.flush_deferred_saves(window, cx);
                            }
                        });
                    }
                },
            )
            .detach();

            let item_id = self.item_id();
            workspace.update_item_dirty_state(self, window, cx);
            cx.observe_release_in(self, window, move |workspace, _, _, _| {
                workspace.panes_by_item.remove(&item_id);
                event_subscription.take();
                send_follower_updates.take();
            })
            .detach();
        }

        cx.defer_in(window, |workspace, window, cx| {
            workspace.serialize_workspace(window, cx);
        });
    }

    fn deactivated(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.deactivated(window, cx));
    }

    fn on_removed(&self, cx: &mut App) { self.update(cx, |item, cx| item.on_removed(cx)); }

    fn workspace_deactivated(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.workspace_deactivated(window, cx));
    }

    fn navigate(&self, data: Arc<dyn Any + Send>, window: &mut Window, cx: &mut App) -> bool {
        self.update(cx, |this, cx| this.navigate(data, window, cx))
    }

    fn item_id(&self) -> EntityId { self.entity_id() }

    fn to_any_view(&self) -> AnyView { self.clone().into() }

    fn is_dirty(&self, cx: &App) -> bool { self.read(cx).is_dirty(cx) }

    fn capability(&self, cx: &App) -> Capability { self.read(cx).capability(cx) }

    fn toggle_read_only(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| {
            this.toggle_read_only(window, cx);
        })
    }

    fn has_deleted_file(&self, cx: &App) -> bool { self.read(cx).has_deleted_file(cx) }

    fn has_conflict(&self, cx: &App) -> bool { self.read(cx).has_conflict(cx) }

    fn can_save(&self, cx: &App) -> bool { self.read(cx).can_save(cx) }

    fn can_save_as(&self, cx: &App) -> bool { self.read(cx).can_save_as(cx) }

    fn save(
        &self,
        options: SaveOptions,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        self.update(cx, |item, cx| item.save(options, project, window, cx))
    }

    fn save_as(
        &self,
        project: Entity<Project>,
        path: ProjectPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        self.update(cx, |item, cx| item.save_as(project, path, window, cx))
    }

    fn reload(
        &self,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        self.update(cx, |item, cx| item.reload(project, window, cx))
    }

    fn act_as_type<'a>(&'a self, type_id: TypeId, cx: &'a App) -> Option<AnyEntity> {
        self.read(cx).act_as_type(type_id, self, cx)
    }

    fn to_followable_item_handle(&self, cx: &App) -> Option<Box<dyn FollowableItemHandle>> {
        FollowableViewRegistry::to_followable_view(self.clone(), cx)
    }

    fn on_release(
        &self,
        cx: &mut App,
        callback: Box<dyn FnOnce(&mut App) + Send>,
    ) -> gpui::Subscription {
        cx.observe_release(self, move |_, cx| callback(cx))
    }

    fn to_searchable_item_handle(&self, cx: &App) -> Option<Box<dyn SearchableItemHandle>> {
        self.read(cx).as_searchable(self, cx)
    }

    fn breadcrumb_location(&self, cx: &App) -> ToolbarItemLocation {
        self.read(cx).breadcrumb_location(cx)
    }

    fn breadcrumbs(&self, cx: &App) -> Option<(Vec<HighlightedText>, Option<Font>)> {
        self.read(cx).breadcrumbs(cx)
    }

    fn breadcrumb_prefix(&self, window: &mut Window, cx: &mut App) -> Option<gpui::AnyElement> {
        self.update(cx, |item, cx| item.breadcrumb_prefix(window, cx))
    }

    fn show_toolbar(&self, cx: &App) -> bool { self.read(cx).show_toolbar() }

    fn pixel_position_of_cursor(&self, cx: &App) -> Option<Point<Pixels>> {
        self.read(cx).pixel_position_of_cursor(cx)
    }

    fn downgrade_item(&self) -> Box<dyn WeakItemHandle> { Box::new(self.downgrade()) }

    fn to_serializable_item_handle(&self, cx: &App) -> Option<Box<dyn SerializableItemHandle>> {
        SerializableItemRegistry::view_to_serializable_item_handle(self.to_any_view(), cx)
    }

    fn preserve_preview(&self, cx: &App) -> bool { self.read(cx).preserve_preview(cx) }

    fn include_in_nav_history(&self) -> bool { T::include_in_nav_history() }

    fn relay_action(&self, action: Box<dyn Action>, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| {
            this.focus_handle(cx).focus(window, cx);
            window.dispatch_action(action, cx);
        })
    }

    /// Called when the containing pane receives a drop on the item or the item's tab.
    /// Returns `true` if the item handled it and the pane should skip its default drop behavior.
    fn handle_drop(
        &self,
        active_pane: &Pane,
        dropped: &dyn Any,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.update(cx, |this, cx| {
            this.handle_drop(active_pane, dropped, window, cx)
        })
    }

    fn tab_extra_context_menu_actions(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<(SharedString, Box<dyn Action>)> {
        self.update(cx, |this, cx| {
            this.tab_extra_context_menu_actions(window, cx)
        })
    }
}
