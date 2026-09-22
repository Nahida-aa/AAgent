use std::collections::HashSet;

use anyhow::{Context as _, Result};
use gpui::{Context, Entity, Task, WeakEntity};

use super::Project;
use crate::image_store::{ImageItem, ImageItemEvent, ImageStoreEvent};
use crate::ProjectPath;

impl Project {
    pub fn open_image(&mut self, path: impl Into<ProjectPath>, cx: &mut Context<Self>) -> Task<Result<Entity<ImageItem>>> { /* 原样 */ }
    pub fn reload_images(&self, images: HashSet<Entity<ImageItem>>, cx: &mut Context<Self>) -> Task<Result<()>> { /* 原样 */ }
    pub(crate) fn on_image_store_event(&mut self, _: Entity<crate::image_store::ImageStore>, event: &ImageStoreEvent, cx: &mut Context<Self>) { /* 原样 */ }
    fn on_image_event(&mut self, image: Entity<ImageItem>, event: &ImageItemEvent, cx: &mut Context<Self>) -> Option<()> { /* 原样 */ }
}
