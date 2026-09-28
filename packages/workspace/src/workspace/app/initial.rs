use super::*;
use gpui::PathPromptOptions;
use settings::DefaultOpenBehavior;
use std::sync::Arc;
use ui::App;

use crate::{
    WorkspaceSettings, history_manager, theme_preview, toast_layer,
    workspace::{
        Workspace,
        app::state::AppState,
        core::actions::{CloseWindow, Open, OpenFiles},
        open::prompt::prompt_for_open_path_and_open,
        serialize::flush::flush_windows_serialization_on_quit,
    },
};

pub fn init(app_state: Arc<AppState>, cx: &mut App) {
    component::init();
    theme_preview::init(cx);
    toast_layer::init(cx);
    history_manager::init(app_state.fs.clone(), cx);

    cx.on_app_quit(flush_windows_serialization_on_quit).detach();

    cx.on_action(|_: &CloseWindow, cx| Workspace::close_global(cx))
        .on_action(|_: &Reload, cx| reload(cx));

    // Register Bubble window phase Open/OpenFiles listeners on every Workspace entity.
    // This MUST be Bubble window phase (not Bubble global) because during dispatch,
    // Window::dispatch_action internally defers and takes the window slot (#2),
    // so Bubble global executes with slot=None → multi_workspace.read() fails.
    // Bubble window phase listeners are on the entity's dispatch path and don't take the slot.
    cx.observe_new({
        let app_state = app_state.clone();
        move |workspace: &mut Workspace, window, cx| {
            let app_state = app_state.clone();
            workspace
                .register_action({
                    let app_state = app_state.clone();
                    move |workspace, action: &Open, window, cx| {
                        let create_new_window = action.create_new_window.unwrap_or_else(|| {
                            matches!(
                                WorkspaceSettings::get_global(cx).default_open_behavior,
                                DefaultOpenBehavior::NewWindow
                            )
                        });
                        prompt_for_open_path_and_open(
                            workspace,
                            app_state.clone(),
                            PathPromptOptions {
                                files: true,
                                directories: true,
                                multiple: true,
                                prompt: None,
                            },
                            create_new_window,
                            window,
                            cx,
                        );
                    }
                })
                .register_action({
                    let app_state = app_state.clone();
                    move |workspace, _: &OpenFiles, window, cx| {
                        let directories = cx.can_select_mixed_files_and_dirs();
                        prompt_for_open_path_and_open(
                            workspace,
                            app_state.clone(),
                            PathPromptOptions {
                                files: true,
                                directories,
                                multiple: true,
                                prompt: None,
                            },
                            true,
                            window,
                            cx,
                        );
                    }
                });
        }
    })
    .detach();
}
