use aa_app_lib::app::AppShell;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowDecorations, WindowOptions, px, size};
use gpui_platform::application;

fn main() {
    tracing_subscriber::fmt::init();

    application().run(|cx: &mut App| {
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
