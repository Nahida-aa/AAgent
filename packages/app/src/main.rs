//! aacode desktop — main entry.
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
    App, AppContext, Menu, MenuItem, NoAction, SharedString,
    WindowBackgroundAppearance, WindowDecorations, px, size,
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
        theme_selector::init(cx); // 注册 Toggle action handler

        // —— Panel init（对齐 Zed zed.rs L6174-6176）——
        git_ui::init(cx);
        project_panel::init(cx);
        outline_panel::init(cx);

        // —— Fs 全局 ——
        <dyn fs::Fs>::set_global(fs.clone(), cx);

        // —— HTTP client（对齐 Zed main.rs L508-L528）——
        // 先给 gpui 一个 ReqwestClient，Client::production 内部要用；
        // 再用 client.http_client()（带 server URL 前缀的 HttpClientWithUrl）覆盖。
        let user_agent = format!("aacode/{}", env!("CARGO_PKG_VERSION"));
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
        // aacode 先传空 channel（None 作为 shell_env_loaded_rx，watch::channel(None) 作为 options）。
        let (_node_options_tx, node_options_rx) = watch::channel(None);
        let node_runtime =
            node_runtime::NodeRuntime::new(client.http_client(), None, node_options_rx);

        // —— Language init（对齐 Zed main.rs L562）——
        // 注册 native grammars（markdown, rust, python, ...）+ LSP adapters
        languages::init(languages.clone(), fs.clone(), node_runtime.clone(), cx);

        // —— Session ——
        let session = cx.foreground_executor().block_on(session_task);
        let session = cx.new(|cx| session::AppSession::new(session, cx));

        // —— UserStore / WorkspaceStore ——
        let user_store = cx.new(|cx| client::UserStore::new(client.clone(), cx));
        let workspace_store = cx.new(|cx| workspace::WorkspaceStore::new(client.clone(), cx));

        // —— Client 全局 ——
        client::Client::set_global(client.clone(), cx);

        // —— Collab 初始化链（对齐 Zed zed.rs L6166-6168）——
        channel::init(&client, user_store.clone(), cx);
        call::init(client.clone(), user_store.clone(), cx);
        notifications::init(client.clone(), user_store.clone(), cx);

        // —— AppState 构造 ——
        let app_state = Arc::new(workspace::AppState {
            languages,
            client,
            user_store,
            workspace_store,
            fs,
            build_window_options: |_, cx| {
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::centered(
                        None,
                        size(1100.0.into(), px(720.0)),
                        cx,
                    ))),
                    window_decorations: Some(WindowDecorations::Client),
                    window_background: cx.theme().window_background_appearance(),
                    titlebar: Some(gpui::TitlebarOptions {
                        appears_transparent: true,
                        title: Some(SharedString::from("aacode")),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            },
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

        // —— Release channel（对齐 Zed main.rs L492）——
        release_channel::init(semver::Version::new(0, 1, 0), cx);

        // —— Workspace 全局 action ——
        workspace::init(app_state.clone(), cx);

        // —— Settings UI（对齐 Zed main.rs L784）——
        settings_ui::init(cx);

        // —— Collab UI（需要 &Arc<AppState>，对齐 Zed zed.rs L6173）——
        collab_ui::init(&app_state, cx);

        // —— Debugger ——
        debugger_ui::init(cx);

        // —— observe_new 注册 ——
        aa_app_lib::initialize::initialize_workspace(app_state.clone(), cx);

        // —— 设置应用菜单（application_menu 靠 cx.get_menus() 读取数据）——
        // Zed 原版由 app_menus.rs 构建完整菜单体系；aacode 还没迁完整，
        // 先提供简化版本让 application_menu 能渲染出来。
        // 放在 open_window 之前因为 title_bar::init 已在上面 observe 了 Workspace。
        cx.set_menus(vec![
            Menu {
                name: "aacode".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("About aacode", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Settings", settings_ui::OpenSettings),
                ],
            },
            Menu {
                name: "File".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("New", NoAction),
                    MenuItem::action("Open", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Save", NoAction),
                    MenuItem::action("Save As", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Close", NoAction),
                    MenuItem::action("Quit", NoAction),
                ],
            },
            Menu {
                name: "Edit".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Undo", NoAction),
                    MenuItem::action("Redo", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Cut", NoAction),
                    MenuItem::action("Copy", NoAction),
                    MenuItem::action("Paste", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Find", NoAction),
                    MenuItem::action("Replace", NoAction),
                ],
            },
            Menu {
                name: "View".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Toggle Terminal", NoAction),
                    MenuItem::action("Toggle Sidebar", NoAction),
                    MenuItem::separator(),
                    MenuItem::action("Themes...", aacode_actions::theme_selector::Toggle { themes_filter: None }),
                    MenuItem::action("Icon Themes...", aacode_actions::icon_theme_selector::Toggle { themes_filter: None }),
                    MenuItem::separator(),
                    MenuItem::action("Zoom In", NoAction),
                    MenuItem::action("Zoom Out", NoAction),
                    MenuItem::action("Reset Zoom", NoAction),
                ],
            },
            Menu {
                name: "Help".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Documentation", NoAction),
                    MenuItem::action("Report Issue", NoAction),
                ],
            },
        ]);

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
        // 对齐 Zed main.rs L877-L953:
        // 1. initialize_workspace 先注册 observe_new
        // 2. cx.spawn 异步启动窗口创建（不 block_on！gpui foreground executor 不能嵌套阻塞）
        // 3. cx.activate(true) 让 app 出现在前台
        cx.spawn({
            let app_state = app_state.clone();
            async move |cx| {
                let _ = cx
                    .update(|cx| {
                        workspace::open_new(
                            workspace::OpenOptions::default(),
                            app_state,
                            cx,
                            |_workspace, _window, _cx| {
                                // Zed 原版会在这里调 Editor::new_file 或 Launchpad
                            },
                        )
                    })
                    .await;
            }
        })
        .detach();

        cx.activate(true);

        // 启动后异步检查 MultiWorkspace 窗口是否可被 read
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                tracing::info!("=== startup post-check ===");
                tracing::info!("total windows: {}", cx.windows().len());
                tracing::info!("active_window: {}", cx.active_window().is_some());
                for (i, window) in cx.windows().iter().enumerate() {
                    let downcast = window.downcast::<workspace::MultiWorkspace>();
                    tracing::info!("  window[{}] downcast:<MultiWorkspace>={}", i, downcast.is_some());
                    if let Some(handle) = downcast {
                        let read_result = handle.read(cx);
                        tracing::info!("  window[{}] .read() is_ok={}", i, read_result.is_ok());
                    }
                }
            });
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            cx.update(|cx| {
                tracing::info!("=== startup post-check (+500ms) ===");
                tracing::info!("active_window: {}", cx.active_window().is_some());
                for (i, window) in cx.windows().iter().enumerate() {
                    let downcast = window.downcast::<workspace::MultiWorkspace>();
                    tracing::info!("  window[{}] downcast:<MultiWorkspace>={}", i, downcast.is_some());
                    if let Some(handle) = downcast {
                        let read_result = handle.read(cx);
                        tracing::info!("  window[{}] .read() is_ok={}", i, read_result.is_ok());
                    }
                }
            });
        })
        .detach();
    });
}
