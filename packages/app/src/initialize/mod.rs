//! observe_new 注册 — 对齐 Zed `crates/zed/src/zed.rs::initialize_workspace`。
//!
//! 关键 insight：Sidebar / TitleBar / Panels 都不在 open_window 里手动创建。
//! 而是在全局 init 之后，通过 `cx.observe_new` 注册回调。
//! 一旦 MultiWorkspace / Workspace entity 被 new，回调自动触发并注入依赖。
//!
//! 之所以要在 `cx.defer` 里创建，是因为 MultiWorkspace::new / Workspace::new 的订阅
//! 链需要先建立好，再注入子 entity（Sidebar 等）。

use gpui::{App, AppContext, Context, Entity, Window};
use std::sync::Arc;

pub mod panels;

/// 注册两个 observe_new：
/// - **MultiWorkspace** → `cx.defer` 里创建 Sidebar + register_sidebar
/// - **Workspace** → 创建 Dock Panel（agent/project/git 等）+ 注册 StatusBar 按钮
pub fn initialize_workspace(_app_state: Arc<workspace::AppState>, cx: &mut App) {
    // —— MultiWorkspace observe_new → Sidebar ——
    cx.observe_new(|multi_workspace: &mut workspace::MultiWorkspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let window_handle = window.window_handle();
        let multi_workspace_handle = cx.entity();

        cx.defer(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    let sidebar = cx.new(|cx| {
                        sidebar::Sidebar::new(multi_workspace_handle.clone(), window, cx)
                    });
                    multi_workspace_handle.update(cx, |mw, cx| {
                        mw.register_sidebar(sidebar, cx);
                    });
                })
                .ok();
        });
    })
    .detach();

    // —— Workspace observe_new → StatusBar 按钮 + Dock Panels ——
    cx.observe_new(|workspace: &mut workspace::Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        register_status_bar_items(workspace, window, cx);

        let panels_task = panels::initialize_panels(window, cx);
        workspace.set_panels_task(panels_task);
    })
    .detach();
}

/// 对齐 Zed `crates/zed/src/zed.rs L602-L652`。
/// 在 Workspace 创建后、initialize_panels 前，把所有非 dock 的状态栏按钮注册进 StatusBar。
///
/// 目前只注册 AAgent 已有 crate 里的类型；缺失 crate（diagnostics, encoding_selector,
/// language_selector, toolchain_selector, language_tools, which_key, line_ending_selector）
/// 需要逐个从 Zed 搬过来后再加。
fn register_status_bar_items(
    workspace: &mut workspace::Workspace,
    window: &mut Window,
    cx: &mut Context<workspace::Workspace>,
) {
    // —— Left side ——
    let search_button = cx.new(|_| search::search_status_button::SearchButton::new());
    let active_file_name = cx.new(|_| workspace::active_file_name::ActiveFileName::new());
    let activity_indicator =
        activity_indicator::ActivityIndicator::new(workspace, window, cx);
    let git_blame_status = cx.new(|_| git_ui::GitBlameStatus::default());
    let merge_conflict_indicator =
        cx.new(|cx| git_ui::MergeConflictIndicator::new(workspace, cx));

    // —— Right side ——
    let cursor_position =
        cx.new(|_| go_to_line::cursor_position::CursorPosition::new(workspace));
    let active_buffer_language =
        cx.new(|_| language_selector::ActiveBufferLanguage::new(workspace));
    let vim_mode_indicator = cx.new(|cx| vim::ModeIndicator::new(window, cx));

    // —— 统一注册进 StatusBar ——
    let status_bar = workspace.status_bar().clone();
    status_bar.update(cx, |status_bar, cx| {
        status_bar.add_left_item(search_button, window, cx);
        status_bar.add_left_item(active_file_name, window, cx);
        status_bar.add_left_item(git_blame_status, window, cx);
        status_bar.add_left_item(merge_conflict_indicator, window, cx);
        status_bar.add_left_item(activity_indicator, window, cx);

        status_bar.add_right_item(cursor_position, window, cx);
        status_bar.add_right_item(active_buffer_language, window, cx);
        // 保持 vim 模式指示器在最右侧（Zed 原版也放在最后）
        status_bar.add_right_item(vim_mode_indicator, window, cx);
    });
}
