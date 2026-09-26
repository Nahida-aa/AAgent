use super::*;
impl Workspace {
    // │   ├── serialize_items
    async fn serialize_items(
        this: &WeakEntity<Self>,
        items_rx: UnboundedReceiver<Box<dyn SerializableItemHandle>>,
        cx: &mut AsyncWindowContext,
    ) -> Result<()> {
        const CHUNK_SIZE: usize = 200;

        let mut serializable_items = items_rx.ready_chunks(CHUNK_SIZE);

        while let Some(items_received) = serializable_items.next().await {
            let unique_items =
                items_received
                    .into_iter()
                    .fold(HashMap::default(), |mut acc, item| {
                        acc.entry(item.item_id()).or_insert(item);
                        acc
                    });

            // We use into_iter() here so that the references to the items are moved into
            // the tasks and not kept alive while we're sleeping.
            for (_, item) in unique_items.into_iter() {
                if let Ok(Some(task)) =
                    this.update(cx, |workspace, cx| item.serialize(workspace, false, cx))
                {
                    cx.background_spawn(async move { task.await.log_err() })
                        .detach();
                }
            }

            cx.background_executor()
                .timer(SERIALIZATION_THROTTLE_TIME)
                .await;
        }

        Ok(())
    }
    // │   └── enqueue_item_serialization
    pub(crate) fn enqueue_item_serialization(
        &mut self,
        item: Box<dyn SerializableItemHandle>,
    ) -> Result<()> {
        self.serializable_items_tx
            .unbounded_send(item)
            .map_err(|err| anyhow!("failed to send serializable item over channel: {err}"))
    }
}
