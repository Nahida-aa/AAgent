use gpui::{Context, Entity};
use language::{Buffer, CursorShape};

use super::Project;
use super::state::AgentLocation;
use crate::Event;

impl Project {
    pub fn set_agent_location(&mut self, new_location: Option<AgentLocation>, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn agent_location(&self) -> Option<AgentLocation> { /* 原样 */ }
}
