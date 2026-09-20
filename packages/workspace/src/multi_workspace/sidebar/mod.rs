//! Sidebar trait（强类型）— 对齐 zed `workspace::Sidebar`。
//!
//! 这是 Sidebar entity 的接口契约。Zed 定义在
//! `crates/workspace/src/multi_workspace.rs` L121。
//!
//! 详细解耦设计见 `docs/sidebar-decoupling.md`。

pub mod handle;
pub mod render_state;

pub use handle::SidebarHandle;
pub use render_state::SidebarRenderState;

use gpui::{App, Context, Pixels, Render};
use settings_content::SidebarSide;

/// Sidebar entity 的强类型接口契约 — Sidebar entity 在独立 `packages/sidebar/` crate
/// 自己实现此 trait。
///
/// MultiWorkspace 通过 `Box<dyn SidebarHandle>`（sidebar_handle.rs）间接持有 Sidebar，
/// bridge impl 把 `Entity<Sidebar>` 自动转为 `SidebarHandle`。
/// 这样 workspace crate 不依赖 sidebar crate（避免循环），sidebar crate 单向
/// 依赖 workspace crate（Sidebar trait 定义在这里）。
///
/// 对齐 zed `workspace::Sidebar`（multi_workspace.rs L121）。
pub trait Sidebar: Render + Sized {
    /// 当前 sidebar 宽度。
    fn width(&self, cx: &App) -> Pixels;
    /// 设置 sidebar 宽度（None = reset 到默认值）。
    fn set_width(&mut self, width: Option<Pixels>, cx: &mut Context<Self>);
    /// 是否有未读通知（Zed 用于在 Sidebar icon 上显示 badge）。
    fn has_notifications(&self, cx: &App) -> bool;
    /// sidebar 在左还是右。
    fn side(&self, cx: &App) -> SidebarSide;

    /// 是否显示 thread list view（vs archive view）。
    /// 默认 true — archive 视图切换由 toggle_archive 内部管理。
    fn is_threads_list_view_active(&self) -> bool { true }
}
