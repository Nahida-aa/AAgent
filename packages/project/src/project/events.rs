use super::*;

use gpui::{Context, Entity};
use remote::RemoteClientEvent;

use super::Project;
use crate::project_settings::{SettingsObserver, SettingsObserverEvent};
use crate::DisableAiSettings;
use crate::{Event, ToastLink};

impl Project {
    pub(crate) fn on_settings_observer_event(
        &mut self,
        _: Entity<SettingsObserver>,
        event: &SettingsObserverEvent,
        cx: &mut Context<Self>,
    ) { /* 原样 */ }

    pub(crate) fn on_remote_client_event(
        &mut self,
        _: Entity<remote::RemoteClient>,
        event: &RemoteClientEvent,
        cx: &mut Context<Self>,
    ) { /* 原样 */ }
}
