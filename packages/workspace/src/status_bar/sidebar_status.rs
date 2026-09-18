//! SidebarStatus — StatusBar 私有的 sidebar 状态快照。
//!
//! 对齐 zed `status_bar.rs` L74-L96。
//! 这不是共享类型 — 只在 StatusBar 内部使用。StatusBar::render() 每帧调
//! `SidebarStatus::query()` 从 MultiWorkspace 实时查询，不缓存。
//!
//! 共享给外部的只读投影是 `workspace::SidebarRenderState`（在 multi_workspace 里）。

use gpui::App;

use crate::multi_workspace::{MultiWorkspace, SidebarRenderState};
use settings_content::SidebarSide;

#[derive(Default, Clone, Copy, Debug)]
pub(super) struct SidebarStatus {
    pub(super) open: bool,
    pub(super) side: SidebarSide,
}

impl SidebarStatus {
    /// 从 MultiWorkspace 查询 sidebar 状态快照 — 对齐 zed L82-L96。
    pub(super) fn query(
        multi_workspace: &Option<gpui::WeakEntity<MultiWorkspace>>,
        cx: &App,
    ) -> Self {
        multi_workspace
            .as_ref()
            .and_then(|mw| mw.upgrade())
            .map(|mw| {
                let mw = mw.read(cx);
                Self {
                    open: mw.sidebar_open(),
                    side: mw.sidebar_side(cx),
                }
            })
            .unwrap_or_default()
    }
}

impl From<SidebarRenderState> for SidebarStatus {
    fn from(state: SidebarRenderState) -> Self {
        Self {
            open: state.open,
            side: state.side,
        }
    }
}
