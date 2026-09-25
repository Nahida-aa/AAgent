use std::sync::Arc;

use client::{ChannelId, Client, ParticipantIndex, User};
use gpui::{AnyEntity, App, Entity, Global, Subscription, Task, Window};
use project::Project;

use crate::Workspace;
use crate::pane::Pane;
use crate::shared_screen::SharedScreen;

pub trait AnyActiveCall {
    fn entity(&self) -> AnyEntity;
    fn is_in_room(&self, _: &App) -> bool;
    fn room_id(&self, _: &App) -> Option<u64>;
    fn channel_id(&self, _: &App) -> Option<ChannelId>;
    fn hang_up(&self, _: &mut App) -> Task<Result<()>>;
    fn unshare_project(&self, _: Entity<Project>, _: &mut App) -> Result<()>;
    fn remote_participant_for_peer_id(&self, _: PeerId, _: &App) -> Option<RemoteCollaborator>;
    fn is_sharing_project(&self, _: &App) -> bool;
    fn is_sharing_screen(&self, _: &App) -> bool;
    fn has_remote_participants(&self, _: &App) -> bool;
    fn local_participant_is_guest(&self, _: &App) -> bool;
    fn client(&self, _: &App) -> Arc<Client>;
    fn share_on_join(&self, _: &App) -> bool;
    fn join_channel(&self, _: ChannelId, _: &mut App) -> Task<Result<bool>>;
    fn room_update_completed(&self, _: &mut App) -> Task<()>;
    fn most_active_project(&self, _: &App) -> Option<(u64, u64)>;
    fn share_project(&self, _: Entity<Project>, _: &mut App) -> Task<Result<u64>>;
    fn join_project(
        &self,
        _: u64,
        _: Arc<LanguageRegistry>,
        _: Arc<dyn Fs>,
        _: &mut App,
    ) -> Task<Result<Entity<Project>>>;
    fn peer_id_for_user_in_room(&self, _: u64, _: &App) -> Option<PeerId>;
    fn subscribe(
        &self,
        _: &mut Window,
        _: &mut Context<Workspace>,
        _: Box<dyn Fn(&mut Workspace, &ActiveCallEvent, &mut Window, &mut Context<Workspace>)>,
    ) -> Subscription;
    fn create_shared_screen(
        &self,
        _: PeerId,
        _: &Entity<Pane>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Entity<SharedScreen>>;
    fn peer_ids_with_video_tracks(&self, _: &App) -> Vec<PeerId>;
}

#[derive(Clone)]
pub struct GlobalAnyActiveCall(pub Arc<dyn AnyActiveCall>);
impl Global for GlobalAnyActiveCall {}

impl GlobalAnyActiveCall {
    pub(crate) fn try_global(cx: &App) -> Option<&Self> { cx.try_global() }

    pub(crate) fn global(cx: &App) -> &Self { cx.global() }
}


