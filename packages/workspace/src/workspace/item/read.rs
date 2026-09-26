use super::*;
impl Workspace {
    //
    pub fn open_item_abs_paths(&self, cx: &App) -> Vec<PathBuf> {
        self.items(cx)
            .filter_map(|item| {
                let project_path = item.project_path(cx)?;
                self.project.read(cx).absolute_path(&project_path, cx)
            })
            .collect()
    }
    // pub fn items
    pub fn items<'a>(&'a self, cx: &'a App) -> impl 'a + Iterator<Item = &'a Box<dyn ItemHandle>> {
        self.panes.iter().flat_map(|pane| pane.read(cx).items())
    }
    // pub fn item_of_type<T>
    pub fn item_of_type<T: Item>(&self, cx: &App) -> Option<Entity<T>> {
        self.items_of_type(cx).max_by_key(|item| item.item_id())
    }
    // pub fn items_of_type<T>
    pub fn items_of_type<'a, T: Item>(
        &'a self,
        cx: &'a App,
    ) -> impl 'a + Iterator<Item = Entity<T>> {
        self.panes
            .iter()
            .flat_map(|pane| pane.read(cx).items_of_type())
    }
    // pub fn active_item
    pub fn active_item(&self, cx: &App) -> Option<Box<dyn ItemHandle>> {
        self.active_pane().read(cx).active_item()
    }
    // pub fn active_item_as<I>
    pub fn active_item_as<I: 'static>(&self, cx: &App) -> Option<Entity<I>> {
        let item = self.active_item(cx)?;
        // Prefer an exact downcast so that we return the active item itself when
        // its concrete type matches, preserving entity identity for callers that
        // compare `entity_id`s. Fall back to `act_as` so that wrapper items (e.g.
        // diff views) resolve to the inner view they expose.
        item.to_any_view()
            .downcast::<I>()
            .ok()
            .or_else(|| item.act_as::<I>(cx))
    }

    // fn active_project_path           // private
    fn active_project_path(&self, cx: &App) -> Option<ProjectPath> {
        self.active_item(cx).and_then(|item| item.project_path(cx))
    }
}
