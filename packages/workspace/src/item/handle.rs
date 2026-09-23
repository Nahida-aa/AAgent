//! ItemHandle trait — Pane 里装的东西统一接口。
//!
//! 对齐 Zed `crates/workspace/src/item.rs`。

use super::events::{ItemBufferKind, ItemEvent, SaveOptions, TabContentParams, TabTooltipContent};
use super::follow::{FollowableItemHandle, WeakFollowableItemHandle};
use super::serializable::SerializableItemHandle;
use super::{Item, WeakItemHandle};
use crate::searchable::SearchableItemHandle;
use crate::toolbar::ToolbarItemLocation;
use crate::workspace_settings::WorkspaceSettings;
use crate::{Pane, Workspace, WorkspaceId};
use ui::{Icon, IconName};
use gpui::{
    Action, AnyElement, AnyEntity, AnyView, App, Entity, EntityId, EventEmitter, FocusHandle, Font,
    IntoElement, Pixels, Point, SharedString, Subscription, Task, WeakEntity, Window,
};
use language::{Capability, HighlightedText};
use project::{Project, ProjectEntryId, ProjectPath};
use smallvec::SmallVec;
use std::any::{Any, TypeId};

use crate::{ItemEvent, ToolbarItemLocation};

pub trait ItemHandle: 'static + Send {
    fn item_focus_handle(&self, cx: &App) -> FocusHandle;
    fn subscribe_to_item_events(
        &self,
        window: &mut Window,
        cx: &mut App,
        handler: Box<dyn Fn(ItemEvent, &mut Window, &mut App)>,
    ) -> gpui::Subscription;
    fn tab_content(&self, params: TabContentParams, window: &Window, cx: &App) -> gpui::AnyElement;
    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString;
    fn suggested_filename(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, window: &Window, cx: &App) -> Option<Icon>;
    fn tab_tooltip_text(&self, cx: &App) -> Option<SharedString>;
    fn tab_tooltip_content(&self, cx: &App) -> Option<TabTooltipContent>;
    fn telemetry_event_text(&self, cx: &App) -> Option<&'static str>;
    fn dragged_tab_content(
        &self,
        params: TabContentParams,
        window: &Window,
        cx: &App,
    ) -> gpui::AnyElement;
    fn project_path(&self, cx: &App) -> Option<ProjectPath>;
    fn project_entry_ids(&self, cx: &App) -> SmallVec<[ProjectEntryId; 3]>;
    fn project_paths(&self, cx: &App) -> SmallVec<[ProjectPath; 3]>;
    fn project_item_model_ids(&self, cx: &App) -> SmallVec<[EntityId; 3]>;
    fn for_each_project_item(
        &self,
        _: &App,
        _: &mut dyn FnMut(EntityId, &dyn project::ProjectItem),
    );
    fn buffer_kind(&self, cx: &App) -> ItemBufferKind;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
    fn can_split(&self, cx: &App) -> bool;
    fn clone_on_split(
        &self,
        workspace_id: Option<WorkspaceId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Option<Box<dyn ItemHandle>>>;
    fn added_to_pane(
        &self,
        workspace: &mut Workspace,
        pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut gpui::Context<Workspace>,
    );
    fn deactivated(&self, window: &mut Window, cx: &mut App);
    fn on_removed(&self, cx: &mut App);
    fn workspace_deactivated(&self, window: &mut Window, cx: &mut App);
    fn navigate(
        &self,
        data: std::sync::Arc<dyn Any + Send>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool;
    fn item_id(&self) -> EntityId;
    fn to_any_view(&self) -> AnyView;
    fn is_dirty(&self, cx: &App) -> bool;
    fn capability(&self, cx: &App) -> Capability;
    fn toggle_read_only(&self, window: &mut Window, cx: &mut App);
    fn has_deleted_file(&self, cx: &App) -> bool;
    fn has_conflict(&self, cx: &App) -> bool;
    fn can_save(&self, cx: &App) -> bool;
    fn can_save_as(&self, cx: &App) -> bool;
    fn save(
        &self,
        options: SaveOptions,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>>;
    fn save_as(
        &self,
        project: Entity<Project>,
        path: ProjectPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>>;
    fn reload(
        &self,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>>;
    fn act_as_type(&self, type_id: TypeId, cx: &App) -> Option<AnyEntity>;
    fn to_followable_item_handle(&self, cx: &App) -> Option<Box<dyn FollowableItemHandle>>;
    fn to_serializable_item_handle(&self, cx: &App) -> Option<Box<dyn SerializableItemHandle>>;
    fn on_release(
        &self,
        cx: &mut App,
        callback: Box<dyn FnOnce(&mut App) + Send>,
    ) -> gpui::Subscription;
    fn to_searchable_item_handle(&self, cx: &App) -> Option<Box<dyn SearchableItemHandle>>;
    fn breadcrumb_location(&self, cx: &App) -> ToolbarItemLocation;
    fn breadcrumbs(&self, cx: &App) -> Option<(Vec<language::HighlightedText>, Option<Font>)>;
    fn breadcrumb_prefix(&self, window: &mut Window, cx: &mut App) -> Option<gpui::AnyElement>;
    fn show_toolbar(&self, cx: &App) -> bool;
    fn pixel_position_of_cursor(&self, cx: &App) -> Option<Point<Pixels>>;
    fn downgrade_item(&self) -> Box<dyn WeakItemHandle>;
    fn workspace_settings<'a>(&self, cx: &'a App) -> &'a WorkspaceSettings;
    fn preserve_preview(&self, cx: &App) -> bool;
    fn include_in_nav_history(&self) -> bool;
    fn relay_action(&self, action: Box<dyn Action>, window: &mut Window, cx: &mut App);
    fn handle_drop(
        &self,
        active_pane: &Pane,
        dropped: &dyn Any,
        window: &mut Window,
        cx: &mut App,
    ) -> bool;
    fn tab_extra_context_menu_actions(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<(SharedString, Box<dyn Action>)>;
    fn can_autosave(&self, cx: &App) -> bool {
        let is_deleted = self.project_entry_ids(cx).is_empty();
        self.is_dirty(cx) && !self.has_conflict(cx) && self.can_save(cx) && !is_deleted
    }
}

impl dyn ItemHandle {
    pub fn downcast<V: 'static>(&self) -> Option<Entity<V>> { self.to_any_view().downcast().ok() }

    pub fn act_as<V: 'static>(&self, cx: &App) -> Option<Entity<V>> {
        self.act_as_type(TypeId::of::<V>(), cx)
            .and_then(|t| t.downcast().ok())
    }
}

impl From<Box<dyn ItemHandle>> for AnyView {
    fn from(val: Box<dyn ItemHandle>) -> Self { val.to_any_view() }
}

impl From<&Box<dyn ItemHandle>> for AnyView {
    fn from(val: &Box<dyn ItemHandle>) -> Self { val.to_any_view() }
}

impl Clone for Box<dyn ItemHandle> {
    fn clone(&self) -> Box<dyn ItemHandle> { self.boxed_clone() }
}

impl<T: Item> WeakItemHandle for WeakEntity<T> {
    fn id(&self) -> EntityId { self.entity_id() }

    fn boxed_clone(&self) -> Box<dyn WeakItemHandle> { Box::new(self.clone()) }

    fn upgrade(&self) -> Option<Box<dyn ItemHandle>> {
        self.upgrade().map(|e| Box::new(e) as Box<dyn ItemHandle>)
    }
}
