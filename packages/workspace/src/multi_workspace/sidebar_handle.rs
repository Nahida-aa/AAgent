//! Sidebar trait + SidebarHandle trait — 对齐 zed multi_workspace.rs。
//!
//! Zed 的两个 trait (L121 + L162):
//! - `Sidebar` — Sidebar entity 自己实现（`crates/sidebar/src/sidebar.rs`）。强类型。
//! - `SidebarHandle` — dyn object，MultiWorkspace 存 `Option<Box<dyn SidebarHandle>>`。
//!   用于解耦：sidebar crate 和 workspace crate 不能互相依赖时的 trait object 桥。
//!
//! AAgent 现阶段 Sidebar entity 还在 workspace crate 内（sidebar/mod.rs），
//! MultiWorkspace 仍然强类型存 `Entity<Sidebar>`。但预先建 trait 以便将来
//! Sidebar 独立 crate（packages/sidebar/）时无缝切换到 dyn。
//!
//! 文件放在 multi_workspace/ 下是因为这两个 trait 是 MultiWorkspace 的
//! 抽象层 — 它们定义了 MultiWorkspace 对 Sidebar 的接口契约。

use gpui::{App, Context, Entity, EntityId, Pixels, Render};
use settings_content::SidebarSide;

/// Sidebar entity 的接口契约 — Sidebar entity 自己实现。
/// 对齐 zed `workspace::Sidebar`（multi_workspace.rs L121）。
///
/// 这是"强类型"层 — MultiWorkspace 如果直接存 `Entity<Sidebar>` 用这个。
pub trait SidebarTrait: Render + Sized {
    /// 当前 sidebar 宽度。
    fn width(&self, cx: &App) -> Pixels;
    /// 设置 sidebar 宽度（None = reset）。
    fn set_width(&mut self, width: Option<Pixels>, cx: &mut Context<Self>);
    /// 是否有未读通知（Zed 用于在 Sidebar icon 上显示 badge）。
    fn has_notifications(&self, cx: &App) -> bool;
    /// sidebar 在左还是右。
    fn side(&self, cx: &App) -> SidebarSide;

    /// 是否显示 thread list view（vs archive view）。
    fn is_threads_list_view_active(&self) -> bool {
        true
    }
}
