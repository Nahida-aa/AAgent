use super::*;

//! SidebarHandle — dyn object 桥接层。
//!
//! 详细解耦设计见 `docs/sidebar-decoupling.md`。
//!
//! 对齐 zed `workspace::SidebarHandle`（multi_workspace.rs L162）+
//! `impl<T: Sidebar> SidebarHandle for Entity<T>`（L191）。

use gpui::{AnyView, App, Entity, EntityId, Pixels};
use settings_content::SidebarSide;

use super::Sidebar;

/// dyn object trait — MultiWorkspace 通过它与 Sidebar entity 交互。
///
/// Sidebar entity 在独立 `packages/sidebar/` crate，workspace crate 不能
/// 依赖 sidebar crate（会循环：sidebar→workspace 单向可以），所以
/// MultiWorkspace 存 `Option<Box<dyn SidebarHandle>>` 而不是 `Entity<Sidebar>`。
///
/// Sidebar crate 的 Entity<Sidebar> 通过下面的 bridge impl 自动成为
/// SidebarHandle，App 层只需 `Box::new(sidebar_entity)` 注入即可。
///
/// 对齐 zed `SidebarHandle`（L162-L180）。
pub trait SidebarHandle: 'static + Send + Sync {
    fn width(&self, cx: &App) -> Pixels;
    fn set_width(&self, width: Option<Pixels>, cx: &mut App);
    fn focus_handle(&self, cx: &App) -> FocusHandle;
    fn focus(&self, window: &mut Window, cx: &mut App);
    fn prepare_for_focus(&self, window: &mut Window, cx: &mut App);
    fn has_notifications(&self, cx: &App) -> bool;
    fn to_any(&self) -> AnyView;
    fn entity_id(&self) -> EntityId;
    fn toggle_thread_switcher(&self, select_last: bool, window: &mut Window, cx: &mut App);
    fn cycle_project(&self, forward: bool, window: &mut Window, cx: &mut App);
    fn cycle_thread(&self, forward: bool, window: &mut Window, cx: &mut App);

    fn is_threads_list_view_active(&self, cx: &App) -> bool;

    fn side(&self, cx: &App) -> SidebarSide;
    fn serialized_state(&self, cx: &App) -> Option<String>;
    fn restore_serialized_state(&self, state: &str, window: &mut Window, cx: &mut App);
}

/// 任何实现了 Sidebar trait 的 Entity 自动是 SidebarHandle。
/// 对齐 zed L191-L220。
///
/// 这是"桥接层"——把强类型 Sidebar 转成 dyn object。
/// Entity::update(cx, |this, cx| ...) 从 &mut App 调 entity 方法，
/// 这是 GPUI 设计的"从外部操作 entity"的 API。
impl<T: Sidebar> SidebarHandle for Entity<T> {
    fn width(&self, cx: &App) -> Pixels { self.read(cx).width(cx) }

    fn set_width(&self, width: Option<Pixels>, cx: &mut App) {
        self.update(cx, |this, cx| this.set_width(width, cx))
    }

    fn focus_handle(&self, cx: &App) -> FocusHandle { self.read(cx).focus_handle(cx) }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn prepare_for_focus(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.prepare_for_focus(window, cx));
    }

    fn has_notifications(&self, cx: &App) -> bool { self.read(cx).has_notifications(cx) }

    fn to_any(&self) -> AnyView { self.clone().into() }

    fn entity_id(&self) -> EntityId { Entity::entity_id(self) }

    fn toggle_thread_switcher(&self, select_last: bool, window: &mut Window, cx: &mut App) {
        let entity = self.clone();
        window.defer(cx, move |window, cx| {
            entity.update(cx, |this, cx| {
                this.toggle_thread_switcher(select_last, window, cx);
            });
        });
    }

    fn cycle_project(&self, forward: bool, window: &mut Window, cx: &mut App) {
        let entity = self.clone();
        window.defer(cx, move |window, cx| {
            entity.update(cx, |this, cx| {
                this.cycle_project(forward, window, cx);
            });
        });
    }

    fn cycle_thread(&self, forward: bool, window: &mut Window, cx: &mut App) {
        let entity = self.clone();
        window.defer(cx, move |window, cx| {
            entity.update(cx, |this, cx| {
                this.cycle_thread(forward, window, cx);
            });
        });
    }

    fn is_threads_list_view_active(&self, cx: &App) -> bool {
        self.read(cx).is_threads_list_view_active()
    }

    fn side(&self, cx: &App) -> SidebarSide { self.read(cx).side(cx) }

    fn serialized_state(&self, cx: &App) -> Option<String> { self.read(cx).serialized_state(cx) }

    fn restore_serialized_state(&self, state: &str, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| {
            this.restore_serialized_state(state, window, cx)
        })
    }
}
