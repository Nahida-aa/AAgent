use super::*;
impl Workspace {
    // pub fn set_titlebar_item, 可以考虑新建 titlebar.rs
    pub fn set_titlebar_item(&mut self, item: AnyView, _: &mut Window, cx: &mut Context<Self>) {
        self.titlebar_item = Some(item);
        cx.notify();
    }
    // pub fn titlebar_item, 可以考虑新建 titlebar.rs
    pub fn titlebar_item(&self) -> Option<AnyView> {
        self.titlebar_item.clone()
    }
}
