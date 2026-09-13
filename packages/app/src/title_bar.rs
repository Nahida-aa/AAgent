use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, Rgba, StatefulInteractiveElement, Styled, Window, WindowButton,
    WindowButtonLayout, px, svg,
};

/// Platform-appropriate title bar height.
///
/// Mirrors `zed_ui::platform_title_bar_height` (ui/src/utils/constants.rs):
/// scales with the window's rem size (1.75x), with a minimum of 34px.
pub fn platform_title_bar_height(window: &Window) -> Pixels {
    (1.75 * window.rem_size()).max(px(34.0))
}

/// A client-side-decoration window control button.
///
/// Zed renders these through `WindowControl` in the `platform_title_bar`
/// crate; we reproduce the visuals with inline SVG data so no asset loader
/// is required.
#[derive(Clone, Copy)]
enum WindowControlKind {
    Minimize,
    Maximize,
    Close,
    Restore,
}

const MINIMIZE_SVG: &str = "<svg width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M4 8H12\" stroke=\"#DCE0E5\" stroke-width=\"1.2\"/></svg>";
const MAXIMIZE_SVG: &str = "<svg width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M11.5 4.5H4.5V11.5H11.5V4.5Z\" stroke=\"#DCE0E5\" stroke-width=\"1.2\"/></svg>";
const RESTORE_SVG: &str = "<svg width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M9.5 6.5H3.5V12.5H9.5V6.5Z\" stroke=\"#DCE0E5\" stroke-width=\"1.2\"/><path d=\"M10 8.5H12.5V3.5H7.5V6\" stroke=\"#DCE0E5\" stroke-width=\"1.2\"/></svg>";
const CLOSE_SVG: &str = "<svg width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M11.5 4.5L4.5 11.5\" stroke=\"#DCE0E5\" stroke-width=\"1.2\" stroke-linecap=\"square\" stroke-linejoin=\"round\"/><path d=\"M4.5 4.5L11.5 11.5\" stroke=\"#DCE0E5\" stroke-width=\"1.2\" stroke-linecap=\"square\" stroke-linejoin=\"round\"/></svg>";

impl WindowControlKind {
    fn svg_data(&self) -> &'static str {
        match self {
            Self::Minimize => MINIMIZE_SVG,
            Self::Maximize => MAXIMIZE_SVG,
            Self::Restore => RESTORE_SVG,
            Self::Close => CLOSE_SVG,
        }
    }
}

/// The application shell's custom title bar, mirroring Zed's client-side
/// decorations on Linux: a draggable strip with self-drawn window controls.
pub struct TitleBar {
    pub focus_handle: FocusHandle,
    pub title: String,
}

impl TitleBar {
    pub fn new(title: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            title: title.into(),
        }
    }

    fn title_bar_color(&self) -> Rgba {
        // `tab_bar_background` equivalent for our custom dark theme.
        gpui::rgb(0x141417)
    }

    /// Left-aligned project name trigger.
    ///
    /// Mirrors zed's `render_project_name`: no project is selected in this
    /// placeholder app, so it renders muted like zed's "Open Recent Project"
    /// state, with a hover affordance to suggest it is clickable.
    fn render_project_name(&self) -> impl IntoElement {
        gpui::div()
            .id("project-name")
            .px_1p5()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(gpui::rgb(0x2e2e33)))
            .child(
                gpui::div()
                    .text_size(gpui::rems(0.7))
                    .text_color(gpui::rgb(0x8a8a92))
                    .child(self.title.clone()),
            )
    }

    /// Right-aligned user menu placeholder (avatar + name).
    ///
    /// Zed shows the signed-in user here via a profile menu; we render a static
    /// placeholder circle so the layout still has the shape of the real app.
    fn render_user_menu(&self) -> impl IntoElement {
        gpui::div()
            .id("user-menu")
            .flex_row()
            .items_center()
            .gap_1()
            .px_1()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(gpui::rgb(0x2e2e33)))
            .child(
                gpui::div()
                    .size(gpui::px(18.0))
                    .rounded_full()
                    .bg(gpui::rgb(0x3a3a40))
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .text_size(gpui::rems(0.625))
                            .text_color(gpui::rgb(0xdbdbe1))
                            .child("A"),
                    ),
            )
            .child(
                gpui::div()
                    .text_size(gpui::rems(0.7))
                    .text_color(gpui::rgb(0x9a9aa2))
                    .child("AAgent"),
            )
    }

    fn render_control_button(&self, kind: WindowControlKind) -> impl IntoElement {
        let id = match &kind {
            WindowControlKind::Minimize => "minimize",
            WindowControlKind::Maximize => "maximize",
            WindowControlKind::Restore => "restore",
            WindowControlKind::Close => "close",
        };
        gpui::div()
            .id(id)
            .w(px(24.0))
            .h(px(24.0))
            .items_center()
            .justify_center()
            .rounded(gpui::rems(0.5))
            .hover(|style| style.bg(gpui::rgb(0x2e2e33)))
            .active(|style| style.bg(gpui::rgb(0x3a3a40)))
            .on_click(move |_event, window, _cx| match kind {
                WindowControlKind::Minimize => window.minimize_window(),
                WindowControlKind::Maximize | WindowControlKind::Restore => window.zoom_window(),
                WindowControlKind::Close => window.remove_window(),
            })
            .child(svg().size_4().flex_none().data(kind.svg_data().as_bytes()))
    }

    fn render_window_controls(&self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let layout = WindowButtonLayout::linux_default();
        let is_maximized = window.is_maximized();

        let buttons = layout
            .right
            .iter()
            .filter_map(|&b| b)
            .map(|b| match b {
                WindowButton::Minimize => self.render_control_button(WindowControlKind::Minimize),
                WindowButton::Maximize => {
                    if is_maximized {
                        self.render_control_button(WindowControlKind::Restore)
                    } else {
                        self.render_control_button(WindowControlKind::Maximize)
                    }
                }
                WindowButton::Close => self.render_control_button(WindowControlKind::Close),
            })
            .collect::<Vec<_>>();

        gpui::div().flex_row().gap_1().children(buttons)
    }

    fn render_titlebar(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let height = platform_title_bar_height(window);
        let bg = self.title_bar_color();

        gpui::div()
            .id("titlebar")
            .flex_row()
            .w_full()
            .h(height)
            .px_2()
            .gap_1()
            .items_center()
            .justify_between()
            .bg(bg)
            .flex_none()
            .border_b_1()
            .border_color(gpui::rgb(0x2a2a2e))
            .on_click(|event, window, _cx| {
                if event.click_count() == 2 && window.is_resizable() {
                    window.zoom_window();
                }
            })
            .on_mouse_down(MouseButton::Left, move |_event, window, _cx| {
                window.start_window_move();
            })
            .on_mouse_down(MouseButton::Right, move |event, window, _cx| {
                window.show_window_menu(event.position);
            })
            .child(self.render_project_name())
            .child(
                gpui::div()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(self.render_user_menu())
                    .child(self.render_window_controls(window, cx)),
            )
    }
}

impl Focusable for TitleBar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_titlebar(window, cx)
    }
}
