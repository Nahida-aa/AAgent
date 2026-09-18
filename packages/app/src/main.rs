use aa_app_lib::app::AppShell;
use gpui::{
    App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, px, size,
};
use gpui_platform::application;

fn main() {
    tracing_subscriber::fmt::init();

    application()
        .with_assets(aa_gpui_kit_assets::Assets)
        .run(|cx: &mut App| {
            aa_gpui_kit_assets::Assets
                .load_fonts(cx)
                .expect("failed to load embedded fonts");
            ui_gpui::theme::init_theme(cx);
            ui_gpui::bind_input_keys(cx);
            ui_gpui::base::input::editor::bind_editor_keys(cx);

            // 设置系统（RustEmbed default.json → gpui Global SettingsStore）
            settings::SettingsStore::init(cx);

            // TerminalPanel action handler 注册（对齐 Zed terminal_view::init(cx)）
            aa_terminal_view::TerminalPanel::init(cx);

            let bounds = Bounds::centered(None, size(1100.0.into(), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 自绘 header：隐藏系统标题栏。Wayland 上强制 CSD（否则合成器给
                    // Server 装饰，与自绘 header 叠双标题栏）；X11/Windows 该字段
                    // 行为受平台管（见 app.rs 的 show_titlebar 判定）。
                    window_decorations: Some(WindowDecorations::Client),
                    // 任务栏 / alt-tab / SSD 标题（自有 header 在客户区内绘制）。
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(SharedString::from("AAgent")),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(AppShell::new),
            )
            .unwrap();
            cx.activate(true);
        });
}
