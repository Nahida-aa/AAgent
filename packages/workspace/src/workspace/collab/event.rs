
use super::*;
pub enum ActiveCallEvent {
    ParticipantLocationChanged { participant_id: PeerId },
    RemoteVideoTracksChanged { participant_id: PeerId },
    LocalScreenShareStarted,
    LocalScreenShareStopped,
    RoomLeft,
}

impl Workspace {
    //
    fn on_active_call_event(
        &mut self,
        event: &ActiveCallEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ActiveCallEvent::ParticipantLocationChanged { participant_id } => {
                self.leader_updated(participant_id, window, cx);
            }
            ActiveCallEvent::RemoteVideoTracksChanged { participant_id } => {
                self.leader_updated(participant_id, window, cx);
                self.handle_auto_watch_video_tracks_changed(*participant_id, window, cx);
            }
            ActiveCallEvent::LocalScreenShareStarted => {
                if let AutoWatch::Active { .. } = self.auto_watch {
                    self.auto_watch = AutoWatch::Paused;
                    cx.notify();
                }
            }
            ActiveCallEvent::LocalScreenShareStopped => {
                self.handle_auto_watch_local_share_stopped(window, cx);
            }
            ActiveCallEvent::RoomLeft => {
                if self.auto_watch.enabled() {
                    self.auto_watch = AutoWatch::Off;
                    cx.notify();
                }
            }
        }
    }
}
