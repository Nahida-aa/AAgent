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
            theme_settings::init_theme(cx);
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
                    // 行为受平台管（见 title-bar crate 的 show_titlebar 判定）。
                    window_decorations: Some(WindowDecorations::Client),
                    // 任务栏 / alt-tab / SSD 标题（自有 header 在客户区内绘制）。
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(SharedString::from("AAgent")),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    // 对齐 Zed: open_window 里直接 new Workspace → MultiWorkspace
                    // MultiWorkspace 是 GPUI 窗口根 entity
                    let workspace = cx.new(|cx| workspace::Workspace::new(cx));
                    let mw = cx.new(|cx| workspace::MultiWorkspace::new(workspace.clone(), cx));

                    // —— 初始化所有 Dock Panel（对齐 Zed initialize_panels）——
                    // 直接传 workspace entity + &mut App，内部自己 update_entity
                    let _ =
                        aa_app_lib::initialize::panels::initialize_panels(window, &workspace, cx);

                    // —— 创建 Sidebar + 注入 MultiWorkspace ——
                    let sidebar = cx.new(|cx| aa_sidebar::Sidebar::new(cx));
                    sidebar.update(cx, |s, cx| {
                        s.set_multi_workspace(mw.clone());
                        cx.notify();
                    });
                    mw.update(cx, |mw, cx| {
                        mw.set_sidebar(Box::new(sidebar.clone()), cx);
                    });

                    // —— Workspace/StatusBar 绑定 MultiWorkspace ——
                    // StatusBar toggle sidebar 走 MultiWorkspace 中转
                    workspace.update(cx, |w, cx| {
                        w.set_multi_workspace(mw.clone(), cx);
                    });

                    // —— 创建 TitleBar + 注入 Workspace ——
                    // 对齐 Zed title_bar::init → workspace.set_titlebar_item
                    let titlebar =
                        cx.new(|cx| aa_title_bar::TitleBar::new("AAgent", workspace.clone(), cx));
                    workspace.update(cx, |w, cx| {
                        w.set_titlebar_item(titlebar.into(), cx);
                    });

                    mw
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
