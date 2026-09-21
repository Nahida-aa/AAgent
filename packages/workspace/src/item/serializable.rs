use std::any::Any;

use gpui::{App, Context, Entity, Task, WeakEntity, Window};

use super::handle::ItemHandle;
use super::traits::Item;
use crate::persistence::model::ItemId;
use crate::{Workspace, WorkspaceId};

pub trait SerializableItem: Item {
    fn serialized_item_kind() -> &'static str;

    fn cleanup(
        workspace_id: WorkspaceId,
        alive_items: Vec<ItemId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>>;

    fn deserialize(
        _project: Entity<project::Project>,
        _workspace: WeakEntity<Workspace>,
        _workspace_id: WorkspaceId,
        _item_id: ItemId,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Entity<Self>>>;

    fn serialize(
        &mut self,
        workspace: &mut Workspace,
        item_id: ItemId,
        closing: bool,
        cx: &mut Context<Self>,
    ) -> Option<Task<anyhow::Result<()>>>;

    fn should_serialize(&self, event: &Self::Event) -> bool;
}

pub trait SerializableItemHandle: ItemHandle {
    fn serialized_item_kind(&self) -> &'static str;
    fn serialize(
        &self,
        workspace: &mut Workspace,
        closing: bool,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<()>>>;
    fn should_serialize(&self, event: &dyn Any, cx: &App) -> bool;
}

impl<T> SerializableItemHandle for Entity<T>
where
    T: SerializableItem,
{
    fn serialized_item_kind(&self) -> &'static str { T::serialized_item_kind() }

    fn serialize(
        &self,
        workspace: &mut Workspace,
        closing: bool,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<()>>> {
        self.update(cx, |this, cx| {
            this.serialize(workspace, cx.entity_id().as_u64(), closing, cx)
        })
    }

    fn should_serialize(&self, event: &dyn Any, cx: &App) -> bool {
        event
            .downcast_ref::<T::Event>()
            .is_some_and(|event| self.read(cx).should_serialize(event))
    }
}
