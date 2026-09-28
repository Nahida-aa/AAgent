//! aacode desktop (`aa-app`) library entry.
//!
//! 模块划分对齐 Zed `crates/zed/src/`:
//! - `core` — 全局 init 链（gpui_tokio / theme / settings / editor / terminal / title_bar / workspace::init / AppState 构建）
//! - `initialize` — observe_new 注册（Sidebar / Panels），对齐 Zed `zed.rs::initialize_workspace`
//! - `reliability` — 可靠性 / 崩溃恢复（暂空）

pub mod core;
pub mod initialize;
pub mod reliability;
