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
    App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use gpui_platform::application;
use std::sync::Arc;
use theme::ActiveTheme;

fn main() {
    tracing_subscriber::fmt::init();

    // `aa-app --printenv` — shell env 捕获子进程（对齐 Zed main.rs L251-L255）。
    // project/src/environment.rs 的 capture_unix 会 shell exec `<exe> --printenv`
    // 来拿到 JSON env vars。没这个分支 shell env 就全是空的。
    if std::env::args().any(|a| a == "--printenv") {
        util::shell_env::print_env();
        return;
    }

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
        // 顺序对齐 Zed: settings::init 必须在 theme_settings::init 之前
        // （theme_settings::init 需要 SettingsStore 存在才能读 ThemeSettings）
        gpui_tokio::init(cx);
        settings::init(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
        terminal_view::init(cx);
        title_bar::init(cx); // 内部 observe_new(|ws| ws.set_titlebar_item)

        // —— Fs 全局 ——
        <dyn fs::Fs>::set_global(fs.clone(), cx);

        // —— HTTP client（对齐 Zed main.rs L508-L528）——
        // 先给 gpui 一个 ReqwestClient，Client::production 内部要用；
        // 再用 client.http_client()（带 server URL 前缀的 HttpClientWithUrl）覆盖。
        let user_agent = format!("AAgent/{}", env!("CARGO_PKG_VERSION"));
        let http = {
            let _guard = gpui_tokio::Tokio::handle(cx).enter();
            reqwest_client::ReqwestClient::proxy_and_user_agent(None, &user_agent)
                .expect("could not start HTTP client")
        };
        cx.set_http_client(Arc::new(http));

        // —— Client ——
        let client = client::Client::production(cx);
        cx.set_http_client(client.http_client());

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

        // —— Agent init 链（对齐 Zed main.rs L694-L722，AppState 之后）——
        language_model::init(cx);
        let prompt_builder = prompt_store::PromptBuilder::load(app_state.fs.clone(), false, cx);
        project::AgentRegistryStore::init_global(
            cx,
            app_state.fs.clone(),
            app_state.client.http_client(),
        );
        agent_ui::init(
            app_state.fs.clone(),
            prompt_builder,
            app_state.languages.clone(),
            false, // is_new_install
            false, // is_eval
            cx,
        );

        // —— Workspace 全局 action ——
        workspace::init(app_state.clone(), cx);

        // —— observe_new 注册 ——
        aa_app_lib::initialize::initialize_workspace(app_state, cx);

        // —— 主题变化时更新所有窗口的 background_appearance（对齐 Zed main.rs L795-L829）——
        // 必须在 open_window 之前注册，这样第一个窗口创建后主题变化也能生效。
        cx.observe_global::<theme::GlobalTheme>(|cx| {
            let background_appearance = cx.theme().window_background_appearance();
            for &mut window in cx.windows().iter_mut() {
                window
                    .update(cx, |_, window, _| {
                        window.set_background_appearance(background_appearance)
                    })
                    .ok();
            }
        })
        .detach();

        // —— 打开第一个窗口 ——
        let bounds = Bounds::centered(None, size(1100.0.into(), px(720.0)), cx);
        let window_background = cx.theme().window_background_appearance();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                window_background,
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
