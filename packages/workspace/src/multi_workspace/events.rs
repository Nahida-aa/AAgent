use super::*;

use gpui::{App, ElementId, Entity, EntityId, WeakEntity};
use settings::Settings;
use settings::SidebarDockPosition;
use ui::{ContextMenu, IconPosition, right_click_menu};

use crate::Workspace;
use agent_settings::AgentSettings;

pub enum MultiWorkspaceEvent {
    ActiveWorkspaceChanged {
        source_workspace: Option<WeakEntity<Workspace>>,
    },
    WorkspaceAdded(Entity<Workspace>),
    WorkspaceRemoved(EntityId),
    ProjectGroupsChanged,
}

pub enum SidebarEvent {
    SerializeNeeded,
}

// 对齐 zed：SidebarRenderState 只在 sidebar/render_state.rs 定义一份，
// 这里原样转出（此前两处各定义了一份，platform_title_bar 经 workspace 根
// 拿到的是 render_state 侧，而 sidebar_ops 用的是 events 侧，E0308）。
pub use super::sidebar::render_state::SidebarRenderState;

pub use settings::SidebarSide;

pub fn sidebar_side_context_menu(
    id: impl Into<ElementId>,
    cx: &App,
) -> ui::RightClickMenu<ContextMenu> {
    let current_position = AgentSettings::get_global(cx).sidebar_side;
    right_click_menu(id).menu(move |window, cx| {
        let fs = <dyn fs::Fs>::global(cx);
        ContextMenu::build(window, cx, move |mut menu, _, _cx| {
            let positions: [(SidebarDockPosition, &str); 2] = [
                (SidebarDockPosition::Left, "Left"),
                (SidebarDockPosition::Right, "Right"),
            ];
            for (position, label) in positions {
                let fs = fs.clone();
                menu = menu.toggleable_entry(
                    label,
                    position == current_position,
                    IconPosition::Start,
                    None,
                    move |_window, cx| {
                        let side = match position {
                            SidebarDockPosition::Left => "left",
                            SidebarDockPosition::Right => "right",
                        };
                        telemetry::event!("Sidebar Side Changed", side = side);
                        settings::update_settings_file(fs.clone(), cx, move |settings, _cx| {
                            settings
                                .agent
                                .get_or_insert_default()
                                .set_sidebar_side(position);
                        });
                    },
                );
            }
            menu
        })
    })
}
