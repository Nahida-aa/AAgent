use super::*;

use std::ops::Range;

use anyhow::Result;
use ::dap::inline_value::{InlineValueLocation, VariableLookupKind, VariableScope};
use crate::debugger::{
    breakpoint_store::ActiveStackFrame,
    dap_store::DapStoreEvent,
    session::Session,
};
use gpui::{Context, Entity, Task};
use language::{Anchor, Buffer, ToPointUtf16};

use super::Project;
use crate::protocol_helpers::provide_inline_values;
use crate::types::InlayHint;

impl Project {
    pub fn active_debug_session(&self, cx: &App) -> Option<(Entity<Session>, ActiveStackFrame)> {
        let active_position = self.breakpoint_store.read(cx).active_position()?;
        let session = self
            .dap_store
            .read(cx)
            .session_by_id(active_position.session_id)?;
        Some((session, active_position.clone()))
    }

    #[inline]
    pub(crate) fn on_dap_store_event(
        &mut self,
        _: Entity<DapStore>,
        event: &DapStoreEvent,
        cx: &mut Context<Self>,
    ) {
        if let DapStoreEvent::Notification(message) = event {
            cx.emit(Event::Toast {
                notification_id: "dap".into(),
                message: message.clone(),
                link: None,
            });
        }
    }
}
