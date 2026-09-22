use super::*;

use std::ops::Range;

use anyhow::Result;
use dap::inline_value::{InlineValueLocation, VariableLookupKind, VariableScope};
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
    pub fn active_debug_session(&self, cx: &gpui::App) -> Option<(Entity<Session>, ActiveStackFrame)> { /* 原样 */ }
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
    pub fn inline_values(&mut self, session: Entity<Session>, active_stack_frame: ActiveStackFrame, buffer_handle: Entity<Buffer>, range: Range<Anchor>, cx: &mut Context<Self>) -> Task<Result<Vec<InlayHint>>> { /* 原样 */ }
}
