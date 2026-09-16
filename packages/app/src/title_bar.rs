//! 自绘窗口头部（title bar），结构对齐 zed 的 `platform_title_bar`。
//!
//! 跨平台策略（整理自 aa-player / packages/aa-player/src/title_bar.rs）：
//! - **Windows**：gpui 窗口本就无系统标题栏，header 恒显示。拖动靠
//!   `WindowControlArea::Drag`：`WM_NCHITTEST` 对拖动区返回 `HTCAPTION` 后由
//!   系统接管（拖动/双击最大化/Win+方向），该路径下 `on_mouse_down` 不会触发，
//!   两条拖动机制不会叠加。
//! - **Wayland**：`WindowOptions.window_decorations = Client` 强制 CSD（否则
//!   KWin 默认给 Server 装饰，会与自绘 header 叠出双标题栏）；拖动用
//!   `start_window_move()`（xdg_toplevel.move），按钮走
//!   `minimize_window` / `zoom_window` / `remove_window`。
//! - **X11/SSD**：`window_decorations()` 返回 `Server`，header 不渲染，交给系统
//!   标题栏（同 zed 默认行为）。
//! - **按钮位置**：跟随桌面环境的 button-layout 设置（zed/aa-player 同款逻辑）
//!   ——gpui 经 xdg-desktop-portal 读取（冒号前=左侧、后=右侧），Windows 固定右侧。
//! - 按钮组容器 `stop_propagation`：按钮按下不能冒泡到整行的拖动处理，否则
//!   Wayland 下点按钮会变成拖窗口（aa-player `window_controls_group` 同款处理）。

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, WindowButton,
    WindowButtonLayout, WindowControlArea, px, svg,
};

#[cfg(target_os = "windows")]
use gpui::MAX_BUTTONS_PER_SIDE;

use ui_gpui::theme::ActiveTheme;

/// Platform-appropriate title bar height.
///
/// Mirrors `zed_ui::platform_title_bar_height` (ui/src/utils/constants.rs):
/// 1.75x the window rem size, minimum 34px (Windows reports a fixed 32px).
pub fn platform_title_bar_height(window: &Window) -> Pixels {
    (1.75 * window.rem_size()).max(px(34.0))
}

/// A client-side-decoration window control button (zed's `generic_*.svg` icons).
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

    fn element_id(&self) -> &'static str {
        match self {
            Self::Minimize => "min",
            Self::Maximize => "max",
            Self::Restore => "restore",
            Self::Close => "close",
        }
    }
}

/// The application shell's custom title bar: a draggable strip with self-drawn
/// window controls, mirroring Zed's client-side decorations on Linux.
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

    fn render_control_button(
        &self,
        kind: WindowControlKind,
        id: String,
        cx: &App,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();
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

    /// Window-control button placement: Windows is always right-aligned
    /// min/max/close; other platforms follow the desktop's button-layout
    /// setting via `cx.button_layout()` (read by gpui through
    /// xdg-desktop-portal), falling back to the right-side Linux default.
    #[cfg(target_os = "windows")]
    fn effective_button_layout(_cx: &App) -> WindowButtonLayout {
        WindowButtonLayout {
            left: [None; MAX_BUTTONS_PER_SIDE],
            right: [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close),
            ],
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn effective_button_layout(cx: &App) -> WindowButtonLayout {
        cx.button_layout()
            .unwrap_or_else(WindowButtonLayout::linux_default)
    }

    /// Render one side of the window controls (minimize/maximize(restore)/close).
    ///
    /// The container stops mouse-down propagation so a button press never
    /// bubbles up to the titlebar's drag handler (aa-player's
    /// `window_controls_group` mirrors this).
    fn render_window_controls_group(
        &self,
        side: &'static str,
        buttons: &[Option<WindowButton>; 3],
        is_maximized: bool,
        cx: &App,
    ) -> AnyElement {
        let items = buttons
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let button = (*b)?;
                let kind = match button {
                    WindowButton::Minimize => WindowControlKind::Minimize,
                    WindowButton::Maximize if is_maximized => WindowControlKind::Restore,
                    WindowButton::Maximize => WindowControlKind::Maximize,
                    WindowButton::Close => WindowControlKind::Close,
                };
                Some(self.render_control_button(
                    kind,
                    format!("{side}-{}-{i}", kind.element_id()),
                    cx,
                ))
            })
            .collect::<Vec<_>>();

        gpui::div()
            .flex_row()
            .items_center()
            .h_full()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(items)
            .into_any_element()
    }

    fn render_titlebar(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let height = platform_title_bar_height(window);
        let bg = self.title_bar_color(cx);
        let colors = cx.theme().colors();
        let layout = Self::effective_button_layout(cx);
        let is_maximized = window.is_maximized();

        let left_controls = (layout.left.iter().any(Option::is_some))
            .then(|| self.render_window_controls_group("tb-l", &layout.left, is_maximized, cx));
        let right_controls =
            self.render_window_controls_group("tb-r", &layout.right, is_maximized, cx);

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
            // 拖动区。Windows：HTCAPTION 截获，鼠标事件不会进应用；
            // Wayland CSD：落到下面的 on_mouse_down，向合成器发起交互式移动。
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, move |_event, window, _cx| {
                window.start_window_move();
            })
            .on_mouse_down(MouseButton::Right, move |event, window, _cx| {
                window.show_window_menu(event.position);
            })
            .on_click(|event, window, _cx| {
                if event.click_count() == 2 && window.is_resizable() {
                    window.zoom_window();
                }
            })
            .children(left_controls.into_iter())
            .child(self.render_project_name(cx))
            .child(
                gpui::div()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(self.render_user_menu(cx))
                    .child(right_controls),
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
