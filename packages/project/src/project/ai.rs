use super::*;

use gpui::{Context, Entity};
use language::{Buffer, CursorShape};

use super::Project;
use super::state::AgentLocation;
use crate::Event;

impl Project {
    pub fn set_agent_location(
        &mut self,
        new_location: Option<AgentLocation>,
        cx: &mut Context<Self>,
    ) {
        if let Some(old_location) = self.agent_location.as_ref() {
            old_location
                .buffer
                .update(cx, |buffer, cx| buffer.remove_agent_selections(cx))
                .ok();
        }

        if let Some(location) = new_location.as_ref() {
            location
                .buffer
                .update(cx, |buffer, cx| {
                    buffer.set_agent_selections(
                        Arc::from([language::Selection {
                            id: 0,
                            start: location.position,
                            end: location.position,
                            reversed: false,
                            goal: language::SelectionGoal::None,
                        }]),
                        false,
                        CursorShape::Hollow,
                        cx,
                    )
                })
                .ok();
        }

        self.agent_location = new_location;
        cx.emit(Event::AgentLocationChanged);
    }
    pub fn agent_location(&self) -> Option<AgentLocation> {
        self.agent_location.clone()
    }
}
