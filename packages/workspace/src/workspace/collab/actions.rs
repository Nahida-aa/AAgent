/// Opens the channel notes for a specific channel by its ID.
#[derive(Clone, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = collab)]
#[serde(deny_unknown_fields)]
pub struct OpenChannelNotesById {
    pub channel_id: u64,
}


actions!(
    collab,
    [
        /// Opens the channel notes for the current call.
        ///
        /// Use `collab_panel::OpenSelectedChannelNotes` to open the channel notes for the selected
        /// channel in the collab panel.
        ///
        /// If you want to open a specific channel, use `zed::OpenZedUrl` with a channel notes URL -
        /// can be copied via "Copy link to section" in the context menu of the channel notes
        /// buffer. These URLs look like `https://zed.dev/channel/channel-name-CHANNEL_ID/notes`.
        OpenChannelNotes,
        /// Mutes your microphone.
        Mute,
        /// Deafens yourself (mute both microphone and speakers).
        Deafen,
        /// Leaves the current call.
        LeaveCall,
        /// Shares the current project with collaborators.
        ShareProject,
        /// Shares your screen with collaborators.
        ScreenShare,
        /// Copies the current room name and session id for debugging purposes.
        CopyRoomId,
    ]
);
