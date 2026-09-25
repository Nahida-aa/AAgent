#[derive(Copy, Clone)]
struct SerializableItemDescriptor {
    deserialize: fn(
        Entity<Project>,
        WeakEntity<Workspace>,
        WorkspaceId,
        ItemId,
        &mut Window,
        &mut Context<Pane>,
    ) -> Task<Result<Box<dyn ItemHandle>>>,
    cleanup: fn(WorkspaceId, Vec<ItemId>, &mut Window, &mut App) -> Task<Result<()>>,
    view_to_serializable_item: fn(AnyView) -> Box<dyn SerializableItemHandle>,
}

#[derive(Default)]
pub(crate) struct SerializableItemRegistry {
    descriptors_by_kind: HashMap<Arc<str>, SerializableItemDescriptor>,
    descriptors_by_type: TypeIdHashMap<SerializableItemDescriptor>,
}

impl Global for SerializableItemRegistry {}

impl SerializableItemRegistry {
    fn deserialize(
        item_kind: &str,
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        workspace_id: WorkspaceId,
        item_item: ItemId,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) -> Task<Result<Box<dyn ItemHandle>>> {
        let Some(descriptor) = Self::descriptor(item_kind, cx) else {
            return Task::ready(Err(anyhow!(
                "cannot deserialize {}, descriptor not found",
                item_kind
            )));
        };

        (descriptor.deserialize)(project, workspace, workspace_id, item_item, window, cx)
    }

    fn cleanup(
        item_kind: &str,
        workspace_id: WorkspaceId,
        loaded_items: Vec<ItemId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let Some(descriptor) = Self::descriptor(item_kind, cx) else {
            return Task::ready(Err(anyhow!(
                "cannot cleanup {}, descriptor not found",
                item_kind
            )));
        };

        (descriptor.cleanup)(workspace_id, loaded_items, window, cx)
    }

    fn view_to_serializable_item_handle(
        view: AnyView,
        cx: &App,
    ) -> Option<Box<dyn SerializableItemHandle>> {
        let this = cx.try_global::<Self>()?;
        let descriptor = this.descriptors_by_type.get(&view.entity_type())?;
        Some((descriptor.view_to_serializable_item)(view))
    }

    fn descriptor(item_kind: &str, cx: &App) -> Option<SerializableItemDescriptor> {
        let this = cx.try_global::<Self>()?;
        this.descriptors_by_kind.get(item_kind).copied()
    }
}

pub fn register_serializable_item<I: SerializableItem>(cx: &mut App) {
    let serialized_item_kind = I::serialized_item_kind();

    let registry = cx.default_global::<SerializableItemRegistry>();
    let descriptor = SerializableItemDescriptor {
        deserialize: |project, workspace, workspace_id, item_id, window, cx| {
            let task = I::deserialize(project, workspace, workspace_id, item_id, window, cx);
            cx.foreground_executor()
                .spawn(async { Ok(Box::new(task.await?) as Box<_>) })
        },
        cleanup: |workspace_id, loaded_items, window, cx| {
            I::cleanup(workspace_id, loaded_items, window, cx)
        },
        view_to_serializable_item: |view| Box::new(view.downcast::<I>().unwrap()),
    };
    registry
        .descriptors_by_kind
        .insert(Arc::from(serialized_item_kind), descriptor);
    registry
        .descriptors_by_type
        .insert(TypeId::of::<I>(), descriptor);
}
