use gpui::{ParentElement, Render, Styled};

/// AAgent desktop app shell.
pub struct AppShell;

impl Render for AppShell {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
            .flex_grow(1.0)
            .bg(gpui::rgb(0x0d1117))
            .justify_center()
            .items_center()
            .child(gpui::div().child("AAgent"))
    }
}
