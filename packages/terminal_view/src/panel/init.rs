use gpui::{App, Context};
use workspace::Workspace;

use super::TerminalPanel;
use super::actions::{Toggle, ToggleFocus};
use super::helpers::is_enabled_in_workspace;

pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, _window, _: &mut Context<Workspace>| {
            workspace.register_action(TerminalPanel::new_terminal);
            workspace.register_action(TerminalPanel::open_terminal);
            workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
                if is_enabled_in_workspace(workspace, cx) {
                    workspace.toggle_panel_focus::<TerminalPanel>(window, cx);
                }
            });
            workspace.register_action(|workspace, _: &Toggle, window, cx| {
                if is_enabled_in_workspace(workspace, cx) {
                    if !workspace.toggle_panel_focus::<TerminalPanel>(window, cx) {
                        workspace.close_panel::<TerminalPanel>(window, cx);
                    }
                }
            });
        },
    )
    .detach();
}
