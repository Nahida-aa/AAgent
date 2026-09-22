use super::*;

use gpui::{Context, Entity};
use ::rpc::proto;
use crate::lsp_store::LspStoreEvent;
use crate::types::*;
use crate::{Event, Project};

impl Project {
    pub(crate) fn on_lsp_store_event(
        &mut self,
        _: Entity<crate::lsp_store::LspStore>,
        event: &LspStoreEvent,
        cx: &mut Context<Self>,
    ) {
        // 原样搬入（原文件里那个超长的 match）
    }
}
