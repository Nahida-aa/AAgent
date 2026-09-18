//! SidebarHandle — dyn object 桥接层。
//!
//! 详细解耦设计见 `docs/sidebar-decoupling.md`。
//!
//! 对齐 zed `workspace::SidebarHandle`（multi_workspace.rs L162）+
//! `impl<T: Sidebar> SidebarHandle for Entity<T>`（L191）。

use gpui::{AnyView, App, Entity, EntityId, Pixels};
use settings_content::SidebarSide;

use super::sidebar::Sidebar;

/// dyn object trait — MultiWorkspace 通过它与 Sidebar entity 交互。
///
/// 当 Sidebar 在独立 crate 时，workspace crate 不能依赖 sidebar crate
/// （循环依赖），所以 MultiWorkspace 存 `Option<Box<dyn SidebarHandle>>`
/// 而不是 `Entity<Sidebar>`。
///
/// AAgent 现在 Sidebar 还在 workspace crate 内（循环依赖不存在），
/// MultiWorkspace 仍然强类型存 `Entity<Sidebar>`。
/// 桥接 impl 已写好，等 Sidebar 独立时只需改字段类型。
///
/// 对齐 zed `SidebarHandle`（L162-L180）。
pub trait SidebarHandle: Send + Sync {
    fn width(&self, cx: &App) -> Pixels;
    fn set_width(&self, width: Option<Pixels>, cx: &mut App);
    fn has_notifications(&self, cx: &App) -> bool;
    fn to_any(&self) -> AnyView;
    fn entity_id(&self) -> EntityId;
    fn side(&self, cx: &App) -> SidebarSide;
}

/// 任何实现了 Sidebar trait 的 Entity 自动是 SidebarHandle。
/// 对齐 zed L191-L220。
///
/// 这是"桥接层"——把强类型 Sidebar 转成 dyn object。
/// Entity::update(cx, |this, cx| ...) 从 &mut App 调 entity 方法，
/// 这是 GPUI 设计的"从外部操作 entity"的 API。
impl<T: Sidebar + 'static> SidebarHandle for Entity<T> {
    fn width(&self, cx: &App) -> Pixels {
        self.read(cx).width(cx)
    }

    fn set_width(&self, width: Option<Pixels>, cx: &mut App) {
        self.update(cx, |this, cx| this.set_width(width, cx))
    }

    fn has_notifications(&self, cx: &App) -> bool {
        self.read(cx).has_notifications(cx)
    }

    fn to_any(&self) -> AnyView {
        self.clone().into()
    }

    fn entity_id(&self) -> EntityId {
        Entity::entity_id(self)
    }

    fn side(&self, cx: &App) -> SidebarSide {
        self.read(cx).side(cx)
    }
}
