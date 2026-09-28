//! AAgent desktop — main entry.
//!
//! 对齐 Zed `crates/zed/src/main.rs`：
//! - `app.run` 里只做三件事：加载字体 → `aa_app_lib::core::init(cx)` → `aa_app_lib::initialize::initialize_workspace(cx)` → `open_window` → `activate`。
//! - Sidebar / TitleBar / Panels 全由 `observe_new` 自动注入。

use gpui::{
    App, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
    px, size,
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

            // —— 全局 init 链 ——
            aa_app_lib::core::init(cx);

            // —— observe_new 注册 ——
            let app_state = workspace::AppState::global(cx);
            aa_app_lib::initialize::initialize_workspace(app_state, cx);

            // —— 打开第一个窗口 ——
            let bounds = Bounds::centered(None, size(1100.0.into(), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_decorations: Some(WindowDecorations::Client),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(SharedString::from("AAgent")),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    // Zed 模式：只 new Workspace → MultiWorkspace
                    // Sidebar / TitleBar / Panels 全由 observe_new 自动注入
                    let app_state = workspace::AppState::global(cx);
                    let project = cx.new(|cx| {
                        project::Project::local(
                            app_state.client.clone(),
                            app_state.node_runtime.clone(),
                            app_state.user_store.clone(),
                            app_state.languages.clone(),
                            app_state.fs.clone(),
                            None,
                            Default::default(),
                            cx,
                        )
                    });
                    let workspace = cx.new(|cx| {
                        workspace::Workspace::new(
                            None,
                            project,
                            app_state.clone(),
                            window,
                            cx,
                        )
                    });
                    cx.new(|cx| workspace::MultiWorkspace::new(workspace, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
