//! Panel trait 定义 + PanelHandle dyn object + 7 个占位 Panel struct。
//!
//! 完全对齐 zed `dock.rs`:
//! - `Panel` trait — 面板 entity 自己实现
//! - `PanelHandle` trait (Send + Sync) — Arc<dyn PanelHandle> 存在 Dock 里
//! - `impl<T: Panel> PanelHandle for Entity<T>` — 自动包装
//!
//! Zed 每个面板（Project、Git、Agent、Terminal...）是独立 Entity，
//! 各自实现 Panel trait，Dock 存 Arc<dyn PanelHandle>。

use std::sync::Arc;

use gpui::{
    App, Context, Entity, EntityId, IntoElement, ParentElement, Pixels, Render, Styled, Window,
    div, prelude::*, px,
};
use settings_content::DockPosition;
use ui_gpui::IconName;

// ---------- Panel trait ----------

/// 面板 entity 实现的 trait。对齐 zed `dock.rs::Panel`。
pub trait Panel: Render + Sized {
    fn panel_key() -> &'static str;
    fn persistent_name() -> &'static str;
    fn default_position(&self, cx: &App) -> DockPosition;
    fn position_is_valid(&self, position: DockPosition) -> bool;
    fn default_size(&self, cx: &App) -> Pixels;
    fn supports_flexible_size(&self) -> bool {
        false
    }
    fn icon(&self, cx: &App) -> IconName;
    fn icon_tooltip(&self, cx: &App) -> &'static str;
    /// 启动时是否自动打开该面板所在 Dock。对齐 zed `Panel::starts_open()`。
    /// Zed: ProjectPanel 默认 true，TerminalPanel 默认 false（settings 可配）。
    fn starts_open(&self, _cx: &App) -> bool {
        false
    }
    /// Dock 开/关 或 active panel 切换时调用。对齐 zed dock.rs:L586-593。
    /// 默认空实现 — 有需要的 panel（如 TerminalPanel）覆盖来 spawn 默认内容。
    fn set_active(&mut self, _active: bool, _cx: &mut Context<Self>) {}
}

// ---------- PanelHandle trait ----------

use std::any::Any;

/// Dock 持有的 trait object。对齐 zed `dock.rs::PanelHandle`。
pub trait PanelHandle: Send + Sync {
    fn panel_id(&self) -> EntityId;
    fn persistent_name(&self) -> &'static str;
    fn panel_key(&self) -> &'static str;
    fn position(&self, cx: &App) -> DockPosition;
    fn position_is_valid(&self, position: DockPosition) -> bool;
    fn default_size(&self, cx: &App) -> Pixels;
    fn supports_flexible_size(&self, cx: &App) -> bool;
    fn icon(&self, cx: &App) -> IconName;
    fn icon_tooltip(&self, cx: &App) -> &'static str;
    /// 转为 AnyView 让 Dock::render 能渲染。对齐 zed `PanelHandle::to_any()`。
    fn to_any(&self) -> gpui::AnyView;
    /// 用于 downcast 回 Entity<T>（Dock::panel() 方法）。
    fn as_any(&self) -> &dyn std::any::Any;
    /// Dock 开/关 或 active panel 切换时调用 set_active。对齐 zed dock.rs:L586-593。
    fn set_active(&self, active: bool, cx: &mut App);
}

impl<T: Panel> PanelHandle for Entity<T> {
    fn panel_id(&self) -> EntityId {
        Entity::entity_id(self)
    }
    fn persistent_name(&self) -> &'static str {
        T::persistent_name()
    }
    fn panel_key(&self) -> &'static str {
        T::panel_key()
    }
    fn position(&self, cx: &App) -> DockPosition {
        self.read(cx).default_position(cx)
    }
    fn position_is_valid(&self, position: DockPosition) -> bool {
        _ = position;
        true
    }
    fn default_size(&self, cx: &App) -> Pixels {
        self.read(cx).default_size(cx)
    }
    fn supports_flexible_size(&self, cx: &App) -> bool {
        self.read(cx).supports_flexible_size()
    }
    fn icon(&self, cx: &App) -> IconName {
        self.read(cx).icon(cx)
    }
    fn icon_tooltip(&self, cx: &App) -> &'static str {
        self.read(cx).icon_tooltip(cx)
    }
    fn to_any(&self) -> gpui::AnyView {
        self.clone().into()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
    fn set_active(&self, active: bool, cx: &mut App) {
        self.update(cx, |panel, cx| panel.set_active(active, cx));
    }
}

// ---------- Project Panel ----------

pub struct ProjectPanel;

impl Render for ProjectPanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("project")
    }
}

impl Panel for ProjectPanel {
    fn panel_key() -> &'static str {
        "project_panel"
    }
    fn persistent_name() -> &'static str {
        "project"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Right
    }
    fn position_is_valid(&self, p: DockPosition) -> bool {
        matches!(p, DockPosition::Left | DockPosition::Right)
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(240.0)
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::FileTree
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Project Panel"
    }
    /// 对齐 Zed ProjectPanel — settings 默认 starts_open: true。
    fn starts_open(&self, _cx: &App) -> bool {
        true
    }
}

// ---------- Git Panel ----------

pub struct GitPanel;

impl Render for GitPanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("git")
    }
}

impl Panel for GitPanel {
    fn panel_key() -> &'static str {
        "git_panel"
    }
    fn persistent_name() -> &'static str {
        "git"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Right
    }
    fn position_is_valid(&self, p: DockPosition) -> bool {
        matches!(p, DockPosition::Left | DockPosition::Right)
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(360.0)
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::GitBranch
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Git Panel"
    }
}

// ---------- Collab Panel ----------

pub struct CollabPanel;

impl Render for CollabPanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("collab")
    }
}

impl Panel for CollabPanel {
    fn panel_key() -> &'static str {
        "collab_panel"
    }
    fn persistent_name() -> &'static str {
        "collab"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Right
    }
    fn position_is_valid(&self, p: DockPosition) -> bool {
        matches!(p, DockPosition::Left | DockPosition::Right)
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(240.0)
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::UserGroup
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Collab Panel"
    }
}

// ---------- Outline Panel ----------

pub struct OutlinePanel;

impl Render for OutlinePanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("outline")
    }
}

impl Panel for OutlinePanel {
    fn panel_key() -> &'static str {
        "outline_panel"
    }
    fn persistent_name() -> &'static str {
        "outline"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Right
    }
    fn position_is_valid(&self, p: DockPosition) -> bool {
        matches!(p, DockPosition::Left | DockPosition::Right)
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(300.0)
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::ListTree
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Outline Panel"
    }
}

// ---------- Agent Panel (flexible) ----------

pub struct AgentPanel;

impl Render for AgentPanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("agent")
    }
}

impl Panel for AgentPanel {
    fn panel_key() -> &'static str {
        "agent"
    }
    fn persistent_name() -> &'static str {
        "agent"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Left
    }
    fn position_is_valid(&self, p: DockPosition) -> bool {
        p != DockPosition::Bottom
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(640.0)
    }
    fn supports_flexible_size(&self) -> bool {
        true
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::ZedAssistant
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Agent Panel"
    }
    /// 非 ProjectPanel 一律默认 false，由 settings 配置开启。
    fn starts_open(&self, _cx: &App) -> bool {
        false
    }
}

// ---------- Terminal Panel ----------
// TerminalPanel 已移到 aa-terminal-view crate。
// 原来的 placeholder 在 panel.rs 是一个空 struct + placeholder render。
// 现在由 terminal-view/src/panel.rs 提供真正的实现（持有 active_pane + new_terminal）。

// ---------- Debug Panel ----------

pub struct DebugPanel;

impl Render for DebugPanel {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        placeholder("debug")
    }
}

impl Panel for DebugPanel {
    fn panel_key() -> &'static str {
        "debug"
    }
    fn persistent_name() -> &'static str {
        "debug"
    }
    fn default_position(&self, _cx: &App) -> DockPosition {
        DockPosition::Bottom
    }
    fn position_is_valid(&self, _p: DockPosition) -> bool {
        true
    }
    fn default_size(&self, _cx: &App) -> Pixels {
        px(320.0)
    }
    fn icon(&self, _cx: &App) -> IconName {
        IconName::Debug
    }
    fn icon_tooltip(&self, _cx: &App) -> &'static str {
        "Debug Panel"
    }
}

// ---------- helper ----------

fn placeholder(name: &str) -> gpui::AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h_full()
        .text_size(px(16.0))
        .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
        .child(format!("{} (placeholder)", name))
        .into_any_element()
}
