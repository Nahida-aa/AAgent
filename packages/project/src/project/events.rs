use super::*;

use gpui::{Context, Entity};
use remote::RemoteClientEvent;
use settings::InvalidSettingsError;

use super::Project;
use crate::DisableAiSettings;
use crate::project_settings::{SettingsObserver, SettingsObserverEvent};
use crate::{Event, ToastLink};

impl Project {
    pub(crate) fn on_settings_observer_event(
        &mut self,
        _: Entity<SettingsObserver>,
        event: &SettingsObserverEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            SettingsObserverEvent::LocalSettingsUpdated(result) => match result {
                Err(InvalidSettingsError::LocalSettings { message, path }) => {
                    let message = format!("Failed to set local settings in {path:?}:\n{message}");
                    cx.emit(Event::Toast {
                        notification_id: format!("local-settings-{path:?}").into(),
                        link: None,
                        message,
                    });
                }
                Ok(path) => cx.emit(Event::HideToast {
                    notification_id: format!("local-settings-{path:?}").into(),
                }),
                Err(_) => {}
            },
            SettingsObserverEvent::LocalTasksUpdated(result) => match result {
                Err(InvalidSettingsError::Tasks { message, path }) => {
                    let message = format!("Failed to set local tasks in {path:?}:\n{message}");
                    cx.emit(Event::Toast {
                        notification_id: format!("local-tasks-{path:?}").into(),
                        link: Some(ToastLink {
                            label: "Open Tasks Documentation",
                            url: "https://zed.dev/docs/tasks",
                        }),
                        message,
                    });
                }
                Ok(path) => cx.emit(Event::HideToast {
                    notification_id: format!("local-tasks-{path:?}").into(),
                }),
                Err(_) => {}
            },
            SettingsObserverEvent::LocalDebugScenariosUpdated(result) => match result {
                Err(InvalidSettingsError::Debug { message, path }) => {
                    let message =
                        format!("Failed to set local debug scenarios in {path:?}:\n{message}");
                    cx.emit(Event::Toast {
                        notification_id: format!("local-debug-scenarios-{path:?}").into(),
                        link: None,
                        message,
                    });
                }
                Ok(path) => cx.emit(Event::HideToast {
                    notification_id: format!("local-debug-scenarios-{path:?}").into(),
                }),
                Err(_) => {}
            },
            SettingsObserverEvent::GlobalTasksUpdated(_)
            | SettingsObserverEvent::GlobalDebugScenariosUpdated(_) => {}
        }
    }

    pub(crate) fn on_remote_client_event(
        &mut self,
        _: Entity<RemoteClient>,
        event: &remote::RemoteClientEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            &remote::RemoteClientEvent::Disconnected { server_not_running } => {
                self.worktree_store.update(cx, |store, cx| {
                    store.disconnected_from_host(cx);
                });
                self.buffer_store.update(cx, |buffer_store, cx| {
                    buffer_store.disconnected_from_host(cx)
                });
                self.lsp_store.update(cx, |lsp_store, _cx| {
                    lsp_store.disconnected_from_ssh_remote()
                });
                cx.emit(Event::DisconnectedFromRemote { server_not_running });
            }
            &remote::RemoteClientEvent::Reconnected => {}
        }
    }
}
