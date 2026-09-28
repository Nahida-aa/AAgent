//! observe_new 注册 — 对齐 Zed `crates/zed/src/zed.rs::initialize_workspace`。
//!
//! 关键 insight：Sidebar / TitleBar / Panels 都不在 open_window 里手动创建。
//! 而是在全局 init 之后，通过 `cx.observe_new` 注册回调。
//! 一旦 MultiWorkspace / Workspace entity 被 new，回调自动触发并注入依赖。
//!
//! 之所以要在 `cx.defer` 里创建，是因为 MultiWorkspace::new / Workspace::new 的订阅
//! 链需要先建立好，再注入子 entity（Sidebar 等）。

use gpui::{App, AppContext};
use std::sync::Arc;

pub mod panels;

/// 注册两个 observe_new：
/// - **MultiWorkspace** → `cx.defer` 里创建 Sidebar + register_sidebar
/// - **Workspace** → 创建 Dock Panel（agent/project/git 等）
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

    // —— Workspace observe_new → Panels ——
    cx.observe_new(|workspace: &mut workspace::Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_handle = cx.entity();
        panels::initialize_panels(window, &workspace_handle, cx);
    })
    .detach();
}
