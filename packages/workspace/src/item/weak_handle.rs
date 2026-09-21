use gpui::{EntityId, WeakEntity};

use super::handle::ItemHandle;
use super::traits::Item;

pub trait WeakItemHandle: Send + Sync {
    fn id(&self) -> EntityId;
    fn boxed_clone(&self) -> Box<dyn WeakItemHandle>;
    fn upgrade(&self) -> Option<Box<dyn ItemHandle>>;
}

impl<T: Item> WeakItemHandle for WeakEntity<T> {
    fn id(&self) -> EntityId { self.entity_id() }

    fn boxed_clone(&self) -> Box<dyn WeakItemHandle> { Box::new(self.clone()) }

    fn upgrade(&self) -> Option<Box<dyn ItemHandle>> {
        self.upgrade().map(|v| Box::new(v) as Box<dyn ItemHandle>)
    }
}
