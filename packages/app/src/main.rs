use aa_app_lib::app::AppShell;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowDecorations, WindowOptions, px, size};
use gpui_platform::application;

fn main() {
    tracing_subscriber::fmt::init();

    application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            assets::Assets
                .load_fonts(cx)
                .expect("failed to load embedded fonts");
            ui_gpui::theme::init_theme(cx);
            ui_gpui::bind_input_keys(cx);
            ui_gpui::base::input::editor::bind_editor_keys(cx);

            let bounds = Bounds::centered(None, size(1100.0.into(), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |_, cx| cx.new(AppShell::new),
            )
            .unwrap();
            cx.activate(true);
        });
}
