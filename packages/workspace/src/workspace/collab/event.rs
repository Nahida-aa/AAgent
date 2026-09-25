
pub enum ActiveCallEvent {
    ParticipantLocationChanged { participant_id: PeerId },
    RemoteVideoTracksChanged { participant_id: PeerId },
    LocalScreenShareStarted,
    LocalScreenShareStopped,
    RoomLeft,
}
