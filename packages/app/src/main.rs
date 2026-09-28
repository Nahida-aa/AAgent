//! AAgent desktop — main entry.
//!
//! 对齐 Zed `crates/zed/src/main.rs` 完整结构：
//! - **app.run 闭包外层**：构建 `AppDatabase`、`RealFs`、启动 `Session::new` async task。
//! - **app.run 闭包内层**：gpui_tokio → theme_settings → settings → editor → terminal → title_bar
//!   → `<dyn Fs>::set_global` → `Client::production` → `LanguageRegistry::new`
//!   → `NodeRuntime::new` → block_on session → `AppSession::new`
//!   → `UserStore::new` / `WorkspaceStore::new`
//!   → `AppState::set_global` → `workspace::init` → `initialize_workspace` → `open_window`。
//! - Sidebar / TitleBar / Panels 全由 `observe_new` 自动注入。

use gpui::{
    App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, px, size,
};
use gpui_platform::application;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt::init();

    // —— app.run 外层：db / fs / session（不依赖 gpui App）——
    let app = application().with_assets(aa_gpui_kit_assets::Assets);
    let app_db = db::AppDatabase::new();
    let fs = fs::RealFs::new(None, app.background_executor());
    let session_id = uuid::Uuid::new_v4().to_string();
    let session_task = app.background_executor().spawn(session::Session::new(
        session_id,
        db::kvp::KeyValueStore::from_app_db(&app_db),
    ));

    app.run(|cx: &mut App| {
        cx.set_global(app_db);

        aa_gpui_kit_assets::Assets
            .load_fonts(cx)
            .expect("failed to load embedded fonts");

        // —— 全局 init 链（纯 crate 初始化，不涉及 AppState）——
        gpui_tokio::init(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        settings::init(cx);
        editor::init(cx);
        terminal_view::init(cx);
        title_bar::init(cx); // 内部 observe_new(|ws| ws.set_titlebar_item)

        // —— Fs 全局 ——
        <dyn fs::Fs>::set_global(fs.clone(), cx);

        // —— Client ——
        let client = client::Client::production(cx);

        // —— LanguageRegistry ——
        let languages = Arc::new(language::LanguageRegistry::new(
            cx.background_executor().clone(),
        ));

        // —— NodeRuntime ——
        // Zed 从 SettingsStore 变化建 watch channel 传 node binary options；
        // AAgent 先传空 channel（None 作为 shell_env_loaded_rx，watch::channel(None) 作为 options）。
        let (_node_options_tx, node_options_rx) = watch::channel(None);
        let node_runtime =
            node_runtime::NodeRuntime::new(client.http_client(), None, node_options_rx);

        // —— Session ——
        let session = cx.foreground_executor().block_on(session_task);
        let session = cx.new(|cx| session::AppSession::new(session, cx));

        // —— UserStore / WorkspaceStore ——
        let user_store = cx.new(|cx| client::UserStore::new(client.clone(), cx));
        let workspace_store = cx.new(|cx| workspace::WorkspaceStore::new(client.clone(), cx));

        // —— Client 全局 ——
        client::Client::set_global(client.clone(), cx);

        // —— Collab call ——
        call::init(client.clone(), user_store.clone(), cx);

        // —— AppState 构造 ——
        let app_state = Arc::new(workspace::AppState {
            languages,
            client,
            user_store,
            workspace_store,
            fs,
            build_window_options: |_, _| Default::default(),
            node_runtime,
            session,
        });
        workspace::AppState::set_global(app_state.clone(), cx);

        // —— Workspace 全局 action ——
        workspace::init(app_state.clone(), cx);

        // —— observe_new 注册 ——
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
                    let project = project::Project::local(
                        app_state.client.clone(),
                        app_state.node_runtime.clone(),
                        app_state.user_store.clone(),
                        app_state.languages.clone(),
                        app_state.fs.clone(),
                        None,
                        Default::default(),
                        cx,
                    );
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
