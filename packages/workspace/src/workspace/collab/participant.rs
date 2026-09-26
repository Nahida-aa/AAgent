/// Workspace-local view of a remote participant's location.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticipantLocation {
    SharedProject { project_id: u64 },
    UnsharedProject,
    External,
}

impl ParticipantLocation {
    pub fn from_proto(location: Option<proto::ParticipantLocation>) -> Result<Self> {
        match location
            .and_then(|l| l.variant)
            .context("participant location was not provided")?
        {
            proto::participant_location::Variant::SharedProject(project) => {
                Ok(Self::SharedProject {
                    project_id: project.id,
                })
            }
            proto::participant_location::Variant::UnsharedProject(_) => Ok(Self::UnsharedProject),
            proto::participant_location::Variant::External(_) => Ok(Self::External),
        }
    }
}
/// Workspace-local view of a remote collaborator's state.
/// This is the subset of `call::RemoteParticipant` that workspace needs.
#[derive(Clone)]
pub struct RemoteCollaborator {
    pub user: Arc<User>,
    pub peer_id: PeerId,
    pub location: ParticipantLocation,
    pub participant_index: ParticipantIndex,
}
