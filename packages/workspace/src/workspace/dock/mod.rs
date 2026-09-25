use gpui::{Context, Entity, Window};

use super::Workspace;
use crate::dock::Dock;

// Dock 是左/右/底三个区域之一，管理开合、当前显示哪个 panel、各 panel 的尺寸状态
// Panel 是具体面板（终端、项目、Outline…），提供默认尺寸和是否可伸缩
// 关系是 1 Dock : N Panel，同一时刻只有 1 个 active panel 可见
// 尺寸状态存在 Dock 里，不在 Panel 里。因为尺寸是容器决定的——受窗口大小、其他 dock、center 列数影响，Panel 自己不知道这些

// left_dock
// right_dock
// bottom_dock
// all_docks
// dock_at_position
// focused_dock_position
// agent_panel_position
mod read;
// add_panel
// remove_panel
// set_bottom_dock_layout     // 写 settings + serialize
mod ops;
// dock 状态的捕获/恢复，和序列化配合
// capture_dock_state
// set_dock_structure
// finish_dock_restoration
mod state;
// 尺寸计算与持久化
// // 遍历 all_docks，找哪个 dock 装了 T panel，取它的 size state
// pub fn panel_size_state<T: Panel>(&self, cx: &App) -> Option<dock::PanelSizeState>;

// // 按 panel_key + workspace_id 从 KVP 读
// pub fn persisted_panel_size_state(&self, panel_key: &'static str, cx: &App) -> Option<dock::PanelSizeState>;

// // 按 panel_key + workspace_id 写 KVP
// pub fn persist_panel_size_state(&self, panel_key: &str, size_state: dock::PanelSizeState, cx: &mut App);

// // 找 dock，调 dock.set_panel_size_state
// pub fn set_panel_size_state<T: Panel>(&mut self, size_state, window, cx) -> bool;
mod sizing;
