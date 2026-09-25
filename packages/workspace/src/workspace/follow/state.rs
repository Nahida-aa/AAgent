/// A stable identifier for a collaborator in the current workspace.
///
/// 是 follow 用的“被跟随者标识”，不是 collab 的参与者对象
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum CollaboratorId {
    PeerId(client::proto::PeerId),
    Agent,
}
impl From<PeerId> for CollaboratorId {
    fn from(peer_id: PeerId) -> Self { CollaboratorId::PeerId(peer_id) }
}

impl From<&PeerId> for CollaboratorId {
    fn from(peer_id: &PeerId) -> Self { CollaboratorId::PeerId(*peer_id) }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewId {
    pub creator: crate::workspace_store::CollaboratorId,
    pub id: u64,
}

pub struct FollowerState {
    pub center_pane: Entity<Pane>,
    pub dock_pane: Option<Entity<Pane>>,
    pub active_view_id: Option<ViewId>,
    pub items_by_leader_view_id: HashMap<ViewId, FollowerView>,
}

pub(crate) struct FollowerView {
    pub view: Box<dyn crate::item::FollowableItemHandle>,
    pub location: Option<client::proto::PanelId>,
}
