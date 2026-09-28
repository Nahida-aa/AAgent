//! 全局 init 链 — 对齐 Zed `crates/zed/src/main.rs` `app.run` 闭包内的初始化顺序。
//!
//! 职责：依次调用各 crate 的 `::init(cx)`，构建并 set_global `AppState`。
//! 不做任何 observe_new 注册 — 那是 `initialize` 模块的事。

use gpui::App;
use std::sync::Arc;

/// 执行所有全局初始化。必须在 `cx.open_window` 之前调用。
pub fn init(cx: &mut App) {
    gpui_tokio::init(cx);

    // —— 主题 ——
    // Zed 用 theme_settings::init(LoadThemes::All(Assets))；AAgent 当前只有 Catppuccin 内置主题。
    // 注意：AppState::test() 内部会再调一次 theme_settings::init(JustBase)，
    // gpui::set_global 覆盖是幂等的，不会有问题。
    theme_settings::init(theme::LoadThemes::JustBase, cx);

    // —— 设置系统 ——
    settings::init(cx);

    // —— 编辑器 / 终端 ——
    editor::init(cx);
    terminal_view::init(cx);

    // —— 标题栏 ——
    title_bar::init(cx); // 内部 observe_new(|ws| ws.set_titlebar_item)

    // —— AppState ——
    // 当前用 test stub（FakeFs + 最小 client），后续替换为生产构建（App::new + RealFs + Client::production）。
    let app_state = workspace::AppState::test(cx);
    workspace::AppState::set_global(app_state.clone(), cx);

    // —— Workspace 全局 action ——
    workspace::init(app_state.clone(), cx);
}
