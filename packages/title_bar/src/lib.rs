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
    AnyElement, App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, WeakEntity,
    Window, WindowButton, WindowButtonLayout, WindowControlArea, px, svg,
};

#[cfg(target_os = "windows")]
use gpui::MAX_BUTTONS_PER_SIDE;

use aa_gpui_kit_theme::ActiveTheme;
use aa_gpui_kit_ui::{ButtonRadius, ButtonStyle, IconButton, IconName};
use workspace::Workspace;

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
    /// Icon in the shared `assets` crate (zed's `generic_*.svg`).
    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::GenericMinimize,
            Self::Maximize => IconName::GenericMaximize,
            Self::Restore => IconName::GenericRestore,
            Self::Close => IconName::GenericClose,
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
    pub app_menu_open: bool,
    pub recent_projects_open: bool,
    /// 弱引用 Workspace — Zed TitleBar 持有它来读取项目名/Git 分支等状态。
    /// AAgent 当前不使用，但预留以便后续扩展（对齐 Zed 架构）。
    pub workspace: WeakEntity<Workspace>,
}

impl TitleBar {
    pub fn new(
        title: impl Into<String>,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            title: title.into(),
            app_menu_open: false,
            recent_projects_open: false,
            workspace: workspace.downgrade(),
        }
    }

    fn title_bar_color(&self, cx: &App) -> gpui::Hsla {
        use aa_gpui_kit_theme::ActiveTheme;
        cx.theme().colors().panel_background
    }

    /// Left-aligned application menu trigger (hamburger icon button).
    ///
    /// Mirrors zed's `ApplicationMenu::render_application_menu`
    /// (application_menu.rs): an icon button with the menu glyph, subtle style,
    /// tooltip "Open Application Menu", opening the client-side app menu.
    fn render_application_menu_trigger(&self, cx: &mut Context<Self>) -> impl IntoElement {
        IconButton::new("application-menu-trigger", IconName::Menu)
            .style(ButtonStyle::Subtle)
            .aria_label("Open Application Menu")
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.app_menu_open = !this.app_menu_open;
            }))
    }

    /// The application menu popover (below the hamburger trigger).
    ///
    /// Placeholder for zed's `ApplicationMenu` menu bar; only About/Quit are
    /// wired up for now.
    fn render_application_menu(&self, top: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let text = colors.text;
        let border = colors.border_variant;
        let hover = colors.element_hover;
        let menu_bg = colors.elevated_surface_background;

        gpui::div()
            .id("application-menu")
            .absolute()
            .top(top)
            .left(gpui::rems(0.5))
            .w(gpui::px(200.0))
            .py(gpui::rems(0.25))
            .rounded_md()
            .bg(menu_bg)
            .border_1()
            .border_color(border)
            .shadow_lg()
            // 弹出层按住不触发标题栏拖动。
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(vec![
                gpui::div()
                    .id("app-menu-about")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(gpui::rems(0.25))
                    .py(gpui::rems(0.125))
                    .gap_1()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|this, _event, _window, _cx| {
                        this.app_menu_open = false;
                    }))
                    .child(gpui::div().text_color(text).child("About AAgent"))
                    .into_any_element(),
                gpui::div()
                    .id("app-menu-quit")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(gpui::rems(0.25))
                    .py(gpui::rems(0.125))
                    .gap_1()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|_this, _event, window, _cx| {
                        window.remove_window();
                    }))
                    .child(gpui::div().text_color(text).child("Quit"))
                    .into_any_element(),
            ])
    }

    /// "Open Recent Project" trigger (zed's `project_name_trigger`).
    ///
    /// A separate left-side button opening the recent-projects popover (zed
    /// `title_bar.rs render_project_name`); muted since no project is open.
    fn render_recent_projects_trigger(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        gpui::div()
            .id("recent-projects-trigger")
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_1p5()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(colors.ghost_element_hover))
            .active(|style| style.bg(colors.ghost_element_active))
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.recent_projects_open = !this.recent_projects_open;
            }))
            .child(
                gpui::div()
                    .text_size(gpui::rems(0.7))
                    .text_color(colors.text_muted)
                    .child("Open Recent Project"),
            )
            .child(
                svg()
                    .size_3p5()
                    .flex_none()
                    .text_color(colors.icon_muted)
                    .path("icons/chevron_down.svg"),
            )
    }

    /// Recent-projects popover. Placeholder list (no projects yet), mirroring
    /// zed's `RecentProjects` popover shape.
    fn render_recent_projects_menu(&self, top: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let text_muted = colors.text_muted;
        let border = colors.border_variant;
        let menu_bg = colors.elevated_surface_background;

        gpui::div()
            .id("recent-projects-menu")
            .absolute()
            .top(top)
            .left(gpui::rems(0.5))
            .w(gpui::px(240.0))
            .py(gpui::rems(0.25))
            .rounded_md()
            .bg(menu_bg)
            .border_1()
            .border_color(border)
            .shadow_lg()
            // 弹出层按住不触发标题栏拖动。
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(vec![
                gpui::div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(gpui::rems(0.5))
                    .pb_1()
                    .child(
                        gpui::div()
                            .text_size(gpui::rems(0.75))
                            .text_color(text_muted)
                            .child("Recent Projects"),
                    )
                    .into_any_element(),
                gpui::div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(gpui::rems(0.25))
                    .py(gpui::rems(0.125))
                    .gap_1()
                    .child(
                        gpui::div()
                            .text_color(text_muted)
                            .child("No Recent Projects"),
                    )
                    .into_any_element(),
            ])
    }

    /// Right-aligned user menu placeholder (avatar + name).
    ///
    /// Zed shows the signed-in user here via a profile menu; we render a static
    /// placeholder circle so the layout still has the shape of the real app.
    fn render_user_menu(&self, cx: &App) -> impl IntoElement {
        let colors = cx.theme().colors();
        gpui::div()
            .id("user-menu")
            .flex()
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
                    .flex()
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

    fn render_control_button(&self, kind: WindowControlKind, id: String) -> impl IntoElement {
        IconButton::new(id, kind.icon())
            // Subtle：hover 浮现 ghost 背景；圆形 hover 由 radius(Full) 提供。
            .style(ButtonStyle::Subtle)
            .radius(ButtonRadius::Full)
            .aria_label(match kind {
                WindowControlKind::Minimize => "Minimize",
                WindowControlKind::Maximize | WindowControlKind::Restore => "Maximize",
                WindowControlKind::Close => "Close",
            })
            .on_click(move |_event, window, _cx| match kind {
                WindowControlKind::Minimize => window.minimize_window(),
                WindowControlKind::Maximize | WindowControlKind::Restore => window.zoom_window(),
                WindowControlKind::Close => window.remove_window(),
            })
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
                Some((kind, format!("{side}-{}-{i}", kind.element_id())))
            })
            .map(|(kind, id)| self.render_control_button(kind, id))
            .collect::<Vec<_>>();

        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .h_full()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(items)
            .into_any_element()
    }

    fn render_titlebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = platform_title_bar_height(window);
        let bg = self.title_bar_color(cx);
        let colors = cx.theme().colors().clone();
        let layout = Self::effective_button_layout(cx);
        let is_maximized = window.is_maximized();

        let left_controls = (layout.left.iter().any(Option::is_some))
            .then(|| self.render_window_controls_group("tb-l", &layout.left, is_maximized));
        let right_controls = self.render_window_controls_group("tb-r", &layout.right, is_maximized);

        let app_menu = self.app_menu_open.then(|| {
            self.render_application_menu(height + px(6.0), cx)
                .into_any_element()
        });
        let recent_projects_menu = self.recent_projects_open.then(|| {
            self.render_recent_projects_menu(height + px(6.0), cx)
                .into_any_element()
        });

        // zed 把「左侧整组」与右侧整组各压成一个 flex 子项，`justify_between`
        // 只在这两组间分配空间——左组永远居左上角，不会因左侧有窗口按钮而居中。
        // 左组顺序同 zed: window controls -> Application Menu -> Open Recent Project。
        let left_side = gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .children(left_controls.into_iter())
            .child(self.render_application_menu_trigger(cx))
            .child(self.render_recent_projects_trigger(cx));

        let right_side = gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .child(self.render_user_menu(cx))
            .child(right_controls);

        gpui::div()
            .id("titlebar")
            .flex()
            .flex_row()
            .relative()
            .w_full()
            .h(height)
            .px_2()
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
            .child(left_side)
            .child(right_side)
            .children(app_menu.into_iter())
            .children(recent_projects_menu.into_iter())
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
