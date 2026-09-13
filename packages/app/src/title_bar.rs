use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, WindowButton,
    WindowButtonLayout, px, svg,
};
use ui_gpui::theme::ActiveTheme;

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
/// crate; we load the same zed-style icons as embedded SVGs from the shared
/// `assets` crate.
#[derive(Clone, Copy)]
enum WindowControlKind {
    Minimize,
    Maximize,
    Close,
    Restore,
}

impl WindowControlKind {
    /// Asset path in the shared `assets` crate (matches zed's `generic_*.svg`).
    fn icon_path(&self) -> &'static str {
        match self {
            Self::Minimize => "icons/generic_minimize.svg",
            Self::Maximize => "icons/generic_maximize.svg",
            Self::Restore => "icons/generic_restore.svg",
            Self::Close => "icons/generic_close.svg",
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

    fn title_bar_color(&self, cx: &App) -> gpui::Hsla {
        use ui_gpui::theme::ActiveTheme;
        cx.theme().colors().panel_background
    }

    /// Left-aligned project name trigger.
    ///
    /// Mirrors zed's `render_project_name`: no project is selected in this
    /// placeholder app, so it renders muted like zed's "Open Recent Project"
    /// state, with a hover affordance to suggest it is clickable.
    fn render_project_name(&self, cx: &App) -> impl IntoElement {
        let colors = cx.theme().colors();
        gpui::div()
            .id("project-name")
            .px_1p5()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(colors.ghost_element_hover))
            .child(
                gpui::div()
                    .text_size(gpui::rems(0.7))
                    .text_color(colors.text_muted)
                    .child(self.title.clone()),
            )
    }

    /// Right-aligned user menu placeholder (avatar + name).
    ///
    /// Zed shows the signed-in user here via a profile menu; we render a static
    /// placeholder circle so the layout still has the shape of the real app.
    fn render_user_menu(&self, cx: &App) -> impl IntoElement {
        let colors = cx.theme().colors();
        gpui::div()
            .id("user-menu")
            .flex_row()
            .items_center()
            .gap_1()
            .px_1()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(colors.ghost_element_hover))
            .child(
                gpui::div()
                    .size(gpui::px(18.0))
                    .rounded_full()
                    .bg(colors.element_background)
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .text_size(gpui::rems(0.625))
                            .text_color(colors.text)
                            .child("A"),
                    ),
            )
            .child(
                gpui::div()
                    .text_size(gpui::rems(0.7))
                    .text_color(colors.text_muted)
                    .child("AAgent"),
            )
    }

    fn render_control_button(&self, kind: WindowControlKind, cx: &App) -> impl IntoElement {
        let colors = cx.theme().colors();
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
            .hover(|style| style.bg(colors.ghost_element_hover))
            .active(|style| style.bg(colors.ghost_element_active))
            .on_click(move |_event, window, _cx| match kind {
                WindowControlKind::Minimize => window.minimize_window(),
                WindowControlKind::Maximize | WindowControlKind::Restore => window.zoom_window(),
                WindowControlKind::Close => window.remove_window(),
            })
            .child(svg().size_4().flex_none().path(kind.icon_path()))
    }

    fn render_window_controls(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = WindowButtonLayout::linux_default();
        let is_maximized = window.is_maximized();

        let buttons = layout
            .right
            .iter()
            .filter_map(|&b| b)
            .map(|b| match b {
                WindowButton::Minimize => {
                    self.render_control_button(WindowControlKind::Minimize, cx)
                }
                WindowButton::Maximize => {
                    if is_maximized {
                        self.render_control_button(WindowControlKind::Restore, cx)
                    } else {
                        self.render_control_button(WindowControlKind::Maximize, cx)
                    }
                }
                WindowButton::Close => self.render_control_button(WindowControlKind::Close, cx),
            })
            .collect::<Vec<_>>();

        gpui::div().flex_row().gap_1().children(buttons)
    }

    fn render_titlebar(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let height = platform_title_bar_height(window);
        let bg = self.title_bar_color(cx);
        let colors = cx.theme().colors();

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
            .border_color(colors.border_variant)
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
            .child(self.render_project_name(cx))
            .child(
                gpui::div()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(self.render_user_menu(cx))
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
