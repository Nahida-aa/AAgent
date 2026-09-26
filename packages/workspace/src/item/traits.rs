pub use language::HighlightedText;

use ui::{Icon, Label, LabelCommon};
use gpui::{
    Action, AnyElement, AnyEntity, App, Context, Entity, EntityId, EventEmitter, Focusable, Font,
    IntoElement, Pixels, Point, Render, SharedString, Task, WeakEntity, Window,
};
use language::Capability;
use project::{Project, ProjectPath};
use std::any::{Any, TypeId};
use std::path::Path;
use std::sync::Arc;

use super::{ItemBufferKind,events::{ ItemEvent, SaveOptions, TabContentParams, TabTooltipContent}};
use crate::{
    ItemNavHistory, Pane, ToolbarItemLocation, Workspace, WorkspaceId,
    invalid_item_view::InvalidItemView, searchable::SearchableItemHandle,
};

pub trait Item: Focusable + EventEmitter<Self::Event> + Render + Sized {
    type Event;

    /// Returns the tab contents.
    ///
    /// By default this returns a [`Label`] that displays that text from
    /// `tab_content_text`.
    fn tab_content(&self, params: TabContentParams, _window: &Window, cx: &App) -> AnyElement {
        let text = self.tab_content_text(params.detail.unwrap_or_default(), cx);

        Label::new(text)
            .single_line()
            .color(params.text_color())
            .into_any_element()
    }

    /// Returns the textual contents of the tab.
    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString;

    /// Returns the suggested filename for saving this item.
    /// By default, returns the tab content text.
    fn suggested_filename(&self, cx: &App) -> SharedString { self.tab_content_text(0, cx) }

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> { None }

    /// Returns the tab tooltip text.
    ///
    /// Use this if you don't need to customize the tab tooltip content.
    fn tab_tooltip_text(&self, _: &App) -> Option<SharedString> { None }

    /// Returns the tab tooltip content.
    ///
    /// By default this returns a Tooltip text from
    /// `tab_tooltip_text`.
    fn tab_tooltip_content(&self, cx: &App) -> Option<TabTooltipContent> {
        self.tab_tooltip_text(cx).map(TabTooltipContent::Text)
    }

    fn to_item_events(_event: &Self::Event, _f: &mut dyn FnMut(ItemEvent)) {}

    fn deactivated(&mut self, _window: &mut Window, _: &mut Context<Self>) {}
    fn discarded(&self, _project: Entity<Project>, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn on_removed(&self, _cx: &mut Context<Self>) {}
    fn workspace_deactivated(&mut self, _window: &mut Window, _: &mut Context<Self>) {}
    fn pane_changed(&mut self, _new_pane_id: EntityId, _cx: &mut Context<Self>) {}
    fn navigate(
        &mut self,
        _: Arc<dyn Any + Send>,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) -> bool {
        false
    }

    fn telemetry_event_text(&self) -> Option<&'static str> { None }

    /// (model id, Item)
    fn for_each_project_item(
        &self,
        _: &App,
        _: &mut dyn FnMut(EntityId, &dyn project::ProjectItem),
    ) {
    }
    fn buffer_kind(&self, _cx: &App) -> ItemBufferKind { ItemBufferKind::None }

    /// Returns the project path that should be treated as active for this item.
    ///
    /// Singleton items use their only project item by default. Items backed by
    /// multiple buffers should override this to return the path for the buffer
    /// under the primary cursor or otherwise selected sub-item.
    fn active_project_path(&self, cx: &App) -> Option<ProjectPath> {
        if self.buffer_kind(cx) != ItemBufferKind::Singleton {
            return None;
        }

        let mut result = None;
        self.for_each_project_item(cx, &mut |_, item| {
            result = item.project_path(cx);
        });
        result
    }

    fn set_nav_history(&mut self, _: ItemNavHistory, _window: &mut Window, _: &mut Context<Self>) {}

    fn can_split(&self) -> bool { false }
    fn clone_on_split(
        &self,
        workspace_id: Option<WorkspaceId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Option<Entity<Self>>>
    where
        Self: Sized,
    {
        _ = (workspace_id, window, cx);
        unimplemented!("clone_on_split() must be implemented if can_split() returns true")
    }
    fn is_dirty(&self, _: &App) -> bool { false }
    fn capability(&self, _: &App) -> Capability { Capability::ReadWrite }

    fn toggle_read_only(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn has_deleted_file(&self, _: &App) -> bool { false }
    fn has_conflict(&self, _: &App) -> bool { false }
    fn can_save(&self, _cx: &App) -> bool { false }
    fn can_save_as(&self, _: &App) -> bool { false }

    fn save(
        &mut self,
        _options: SaveOptions,
        _project: Entity<Project>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        unimplemented!("save() must be implemented if can_save() returns true")
    }
    fn save_as(
        &mut self,
        _project: Entity<Project>,
        _path: ProjectPath,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        unimplemented!("save_as() must be implemented if can_save() returns true")
    }
    fn reload(
        &mut self,
        _project: Entity<Project>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        unimplemented!("reload() must be implemented if can_save() returns true")
    }

    fn act_as_type<'a>(
        &'a self,
        type_id: TypeId,
        self_handle: &'a Entity<Self>,
        _: &'a App,
    ) -> Option<AnyEntity> {
        if TypeId::of::<Self>() == type_id {
            Some(self_handle.clone().into())
        } else {
            None
        }
    }

    fn as_searchable(&self, _: &Entity<Self>, _: &App) -> Option<Box<dyn SearchableItemHandle>> {
        None
    }

    fn breadcrumb_location(&self, _: &App) -> ToolbarItemLocation { ToolbarItemLocation::Hidden }

    fn breadcrumbs(&self, _cx: &App) -> Option<(Vec<HighlightedText>, Option<Font>)> { None }

    /// Returns optional elements to render to the left of the breadcrumb.
    fn breadcrumb_prefix(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        None
    }

    fn added_to_workspace(
        &mut self,
        _workspace: &mut Workspace,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn show_toolbar(&self) -> bool { true }

    fn pixel_position_of_cursor(&self, _: &App) -> Option<Point<Pixels>> { None }

    fn preserve_preview(&self, _cx: &App) -> bool { false }

    fn include_in_nav_history() -> bool { true }

    /// Called when the containing pane receives a drop on the item or the item's tab.
    /// Returns `true` to consume it and suppress the pane's default drop behavior.
    fn handle_drop(
        &self,
        _active_pane: &Pane,
        _dropped: &dyn Any,
        _window: &mut Window,
        _cx: &mut App,
    ) -> bool {
        false
    }

    /// Returns additional actions to add to the tab's context menu.
    /// Each entry is a label and an action to dispatch.
    fn tab_extra_context_menu_actions(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Vec<(SharedString, Box<dyn Action>)> {
        Vec::new()
    }
}
