use super::*;
impl Workspace {
    //
    /// Renders the center pane group wrapped in a `Main` landmark so assistive
    /// technology recognizes the editor as the main region and can navigate to
    /// it. While a screen reader is active the wrapper is also the focus target
    /// for region navigation (it carries the landmark role and label), so
    /// focusing it announces "Editor" instead of falling back to the whole
    /// window. We only make it focusable in that case so it never adds a hitbox
    /// or intercepts mouse focus for other users.
    fn render_center(
        &self,
        render_cx: &PaneRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        div()
            .id("editor-region")
            .role(gpui::Role::Main)
            .aria_label("Editor")
            .when(window.is_a11y_active(), |this| {
                this.track_focus(&self.region_focus_handles.editor)
            })
            .size_full()
            .child(self.center.render(
                self.zoomed.as_ref(),
                self.maximized_pane.as_ref(),
                render_cx,
                window,
                cx,
            ))
    }
}
