use super::*;
impl Workspace {
    fn shared_screen_for_peer(
        &self,
        peer_id: PeerId,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<SharedScreen>> {
        self.active_call()?
            .create_shared_screen(peer_id, pane, window, cx)
    }
}
