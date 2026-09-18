//! Sidebar trait + SidebarHandle trait — 对齐 zed multi_workspace.rs。
//!
//! ## 为什么需要 trait 解耦？（核心问题）
//!
//! Sidebar entity 未来会独立到 `packages/sidebar/` crate（对齐 zed `crates/sidebar`）。
//! 独立后会遇到循环依赖：
//!
//! ```
//! sidebar crate  ──依赖──▶  workspace crate  （Sidebar 实现 Sidebar trait）
//! workspace crate ──依赖──▶  sidebar crate  （MultiWorkspace 存 Sidebar entity）
//! ===========================================
//!                  循环 ❌
//! ```
//!
//! Rust Cargo 不允许 crate 间循环依赖。所以必须在 `workspace` crate 里定义好
//! Sidebar 的接口契约（trait），让 `sidebar` crate 只实现契约、不反向依赖
//! `workspace` crate 的具体类型。
//!
//! ## Zed 的解法：两层 trait
//!
//! Zed 选择了 **dyn object + 两层 trait**：
//!
//! ```rust
//! // 1. 强类型 trait — Sidebar entity 自己实现
//! //    workspace crate 定义，sidebar crate impl（单向依赖 OK）
//! pub trait Sidebar: Focusable + Render + Sized { ... }
//!
//! // 2. dyn object trait — MultiWorkspace 存 Box<dyn SidebarHandle>
//! //    因为 MultiWorkspace 不能直接存 Entity<Sidebar>（循环）
//! pub trait SidebarHandle: Send + Sync { ... }
//!
//! // 3. 桥接：任何 Sidebar 的 Entity 自动是 SidebarHandle
//! impl<T: Sidebar> SidebarHandle for Entity<T> { ... }
//! ```
//!
//! MultiWorkspace:
//! ```rust
//! pub struct MultiWorkspace {
//!     sidebar: Option<Box<dyn SidebarHandle>>,  // dyn，不依赖 sidebar crate
//! }
//! ```
//!
//! 这样 workspace crate 只依赖 **trait 定义**（自己内部的），
//! sidebar crate 依赖 workspace crate 的 trait 定义（单向），
//! 没有循环。
//!
//! ## 其他可选解法（未被 Zed 采用）
//!
//! | 方案 | 描述 | 优缺点 |
//! |---|---|---|
//! | **A. trait object** | Zed 的做法。`Box<dyn SidebarHandle>` | ✅ 解耦彻底；❌ 动态分发开销 + 失去 Entity 强类型 |
//! | **B. 泛型 MultiWorkspace<S: Sidebar>** | MultiWorkspace 模板化 | ❌ 污染所有上层类型（AppShell 也要泛型）；❌ Rust 不支持 trait object + 泛型混合 |
//! | **C. Sidebar 不独立 crate** | 留在 workspace/src/sidebar/ | ✅ 零开销；❌ 违反单一职责；❌ 阻碍独立演进 |
//! | **D. 第三 trait crate** | `sidebar-trait` crate 只放 trait | ❌ 过度工程；❌ Zed 没这么做 |
//!
//! Zed 选 A 是因为 sidebar crate 足够大（~8000 行）、独立演进需求强，
//! dyn 的开销在 UI 层可以忽略。
//!
//! ## AAgent 当前状态
//!
//! **暂用方案 C** — Sidebar entity 还在 workspace/src/sidebar/ 子模块。
//! 循环依赖不存在，MultiWorkspace 强类型存 `Entity<Sidebar>`。
//!
//! **预先定义 SidebarTrait**（本文件），等 Sidebar 迁到 `packages/sidebar/` 时：
//! 1. sidebar crate impl SidebarTrait
//! 2. 本文件加 SidebarHandle (dyn object) 桥接层
//! 3. MultiWorkspace 改成存 `Option<Box<dyn SidebarHandle>>`
//!
//! 迁移路径已铺平 — 只需加桥接层 + 改字段类型。
//!
//! ## Zed 参考
//!
//! - `workspace::Sidebar` trait — multi_workspace.rs L121
//! - `workspace::SidebarHandle` trait — multi_workspace.rs L162
//! - `impl<T: Sidebar> SidebarHandle for Entity<T>` — L191
//! - `MultiWorkspace.sidebar: Option<Box<dyn SidebarHandle>>` — L316

use gpui::{App, Context, Entity, Pixels, Render};
use settings_content::SidebarSide;

/// Sidebar entity 的接口契约 — Sidebar entity 自己实现。
/// 对齐 zed `workspace::Sidebar`（multi_workspace.rs L121）。
///
/// 这是"强类型"层 — 当 Sidebar 还在 workspace crate 内时，
/// MultiWorkspace 可以直接存 `Entity<Sidebar>` 并调此 trait 的方法。
/// 当 Sidebar 迁到独立 crate 后，SidebarHandle dyn object 桥接层会接管。
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
