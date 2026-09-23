use super::*;

use collections::HashSet;

use anyhow::{Context as _, Result, anyhow};
use gpui::{AppContext, Context, Entity, Task, TaskExt, WeakEntity};

use super::Project;
use crate::ProjectPath;
use crate::image_store::{ImageItem, ImageItemEvent, ImageStoreEvent};

impl Project {
    pub fn open_image(
        &mut self,
        path: impl Into<ProjectPath>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<ImageItem>>> {
        if self.is_disconnected(cx) {
            return Task::ready(Err(anyhow!(ErrorCode::Disconnected)));
        }

        let open_image_task = self.image_store.update(cx, |image_store, cx| {
            image_store.open_image(path.into(), cx)
        });

        let weak_project = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let image_item = open_image_task.await?;

            // Check if metadata already exists (e.g., for remote images)
            let needs_metadata =
                cx.read_entity(&image_item, |item, _| item.image_metadata.is_none());

            if needs_metadata {
                let project = weak_project.upgrade().context("Project dropped")?;
                let metadata =
                    ImageItem::load_image_metadata(image_item.clone(), project, cx).await?;
                image_item.update(cx, |image_item, cx| {
                    image_item.image_metadata = Some(metadata);
                    cx.emit(ImageItemEvent::MetadataUpdated);
                });
            }

            Ok(image_item)
        })
    }
    pub fn reload_images(
        &self,
        images: HashSet<Entity<ImageItem>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.image_store
            .update(cx, |image_store, cx| image_store.reload_images(images, cx))
    }
    pub(crate) fn on_image_store_event(
        &mut self,
        _: Entity<ImageStore>,
        event: &ImageStoreEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ImageStoreEvent::ImageAdded(image) => {
                cx.subscribe(image, |this, image, event, cx| {
                    this.on_image_event(image, event, cx);
                })
                .detach();
            }
        }
    }
    fn on_image_event(
        &mut self,
        image: Entity<ImageItem>,
        event: &ImageItemEvent,
        cx: &mut Context<Self>,
    ) -> Option<()> {
        // TODO: handle image events from remote
        if let ImageItemEvent::ReloadNeeded = event
            && !self.is_via_collab()
        {
            self.reload_images([image].into_iter().collect(), cx)
                .detach_and_log_err(cx);
        }

        None
    }
}
